//! IdentityStore DB-backed resolution (M2 single-node restart reliability).
//!
//! Production never populates the in-memory IdentityStore, so per-message
//! signature verification resolved no keys and every delivered message carried
//! `verified: false`. Resolution must fall back to the durable accounts store
//! (`accounts` + `account_keys`) — which also survives restarts for free.
//!
//! Requires PostgreSQL: set TEST_DATABASE_URL or runs against the default dev DB.

use ainm_server::accounts;
use ainm_server::identity::IdentityStore;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use sqlx::PgPool;
use uuid::Uuid;

async fn test_pool() -> Option<PgPool> {
    let url = std::env::var("TEST_DATABASE_URL")
        .or_else(|_| std::env::var("AINM_DATABASE__URL"))
        .unwrap_or_else(|_| "postgres://ainm:dev_password@localhost:5432/ainm".to_string());

    let pool = match sqlx::PgPool::connect(&url).await {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Skipping DB test (no PostgreSQL): {e}");
            return None;
        }
    };
    let migrations_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations");
    sqlx::migrate::Migrator::new(migrations_dir)
        .await
        .expect("Failed to load migrations")
        .run(&pool)
        .await
        .expect("Failed to run migrations");
    Some(pool)
}

fn unique_username() -> String {
    format!("iduser{:x}", Uuid::new_v4().as_fields().0)
}

#[tokio::test]
async fn resolves_account_key_from_db_simulating_restart() {
    let Some(pool) = test_pool().await else {
        return;
    };

    // An account + bound key persisted by a "previous server process".
    // email_hash and public_key carry UNIQUE indexes, so both must be
    // per-run unique — the dev database persists across test runs.
    let username = unique_username();
    let acct = accounts::create_pending_account(
        &pool,
        &username,
        "test.ainm.local",
        &format!("ehash-{username}"),
    )
    .await
    .unwrap();
    accounts::set_status(&pool, acct.id, "active")
        .await
        .unwrap();
    let key_bytes: Vec<u8> = Uuid::new_v4()
        .as_bytes()
        .iter()
        .chain(Uuid::new_v4().as_bytes())
        .copied()
        .collect();
    let key_b64 = URL_SAFE_NO_PAD.encode(&key_bytes);
    accounts::insert_account_key(&pool, acct.id, &key_b64, None)
        .await
        .unwrap();

    // A fresh store (post-restart): empty memory, DB attached.
    let store = IdentityStore::new().with_db(pool.clone());
    let resolved = store.resolve_public_key_bytes(&acct.address).await;
    assert_eq!(
        resolved,
        Some(key_bytes),
        "must resolve from durable account_keys"
    );
}

#[tokio::test]
async fn memory_registration_wins_over_db() {
    let Some(pool) = test_pool().await else {
        return;
    };

    let store = IdentityStore::new().with_db(pool.clone());
    let mem_key: Vec<u8> = vec![7u8; 32];
    store.register(
        "preseed@remote.local",
        &URL_SAFE_NO_PAD.encode(&mem_key),
        "agent",
        "remote.local",
        None,
    );
    let resolved = store.resolve_public_key_bytes("preseed@remote.local").await;
    assert_eq!(
        resolved,
        Some(mem_key),
        "in-memory pre-seeds resolve without DB hit"
    );
}

#[tokio::test]
async fn unknown_address_resolves_to_none() {
    let Some(pool) = test_pool().await else {
        return;
    };
    let store = IdentityStore::new().with_db(pool);
    assert_eq!(
        store.resolve_public_key_bytes("ghost@nowhere.local").await,
        None
    );
}

#[tokio::test]
async fn no_db_store_falls_back_to_memory_only() {
    let store = IdentityStore::new();
    assert_eq!(store.resolve_public_key_bytes("anyone@x.local").await, None);
    let key: Vec<u8> = vec![1u8; 32];
    store.register(
        "a@x.local",
        &URL_SAFE_NO_PAD.encode(&key),
        "human",
        "x.local",
        None,
    );
    assert_eq!(store.resolve_public_key_bytes("a@x.local").await, Some(key));
}
