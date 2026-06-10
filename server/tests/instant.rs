//! Instant agent registration (M5 fast path): one POST creates an active agent
//! account with a bound key — the signed WS handshake then works unchanged.
//! Requires PostgreSQL: set TEST_DATABASE_URL or runs against the default dev DB.

use ainm_server::{accounts, instant};
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

fn fresh_key() -> String {
    let bytes: Vec<u8> = Uuid::new_v4()
        .as_bytes()
        .iter()
        .chain(Uuid::new_v4().as_bytes())
        .copied()
        .collect();
    URL_SAFE_NO_PAD.encode(bytes)
}

fn unique_name() -> String {
    format!("bot{:x}", Uuid::new_v4().as_fields().0)
}

#[tokio::test]
async fn registers_active_account_with_bound_key() {
    let Some(pool) = test_pool().await else {
        return;
    };
    let name = unique_name();
    let key = fresh_key();

    let address = instant::instant_register(&pool, "test.ainm.local", &name, &key)
        .await
        .unwrap();
    assert_eq!(address, format!("{name}@test.ainm.local"));

    // The WS handshake's exact lookups must succeed against this account.
    let acct = accounts::find_account_by_address(&pool, &address)
        .await
        .unwrap()
        .expect("account exists");
    assert_eq!(acct.status, "active");
    let bound = accounts::find_active_key(&pool, acct.id)
        .await
        .unwrap()
        .expect("key bound");
    assert_eq!(bound.public_key, key);
}

#[tokio::test]
async fn duplicate_name_is_suffixed_not_rejected() {
    let Some(pool) = test_pool().await else {
        return;
    };
    let name = unique_name();
    let a = instant::instant_register(&pool, "test.ainm.local", &name, &fresh_key())
        .await
        .unwrap();
    let b = instant::instant_register(&pool, "test.ainm.local", &name, &fresh_key())
        .await
        .unwrap();
    assert_ne!(a, b);
    assert!(b.starts_with(&format!("{name}-")), "suffixed: {b}");
}

#[tokio::test]
async fn rejects_invalid_key_and_name() {
    let Some(pool) = test_pool().await else {
        return;
    };
    // Key must be 32 base64url bytes.
    assert!(matches!(
        instant::instant_register(&pool, "d.local", &unique_name(), "tooshort").await,
        Err(instant::InstantError::InvalidKey)
    ));
    // Name must survive sanitization.
    assert!(matches!(
        instant::instant_register(&pool, "d.local", "!!!", &fresh_key()).await,
        Err(instant::InstantError::InvalidName)
    ));
}

#[test]
fn sanitize_name_rules() {
    assert_eq!(instant::sanitize_name("My Bot"), Some("my-bot".into()));
    assert_eq!(instant::sanitize_name("ok-bot-9"), Some("ok-bot-9".into()));
    assert_eq!(instant::sanitize_name("!!!"), None);
    assert_eq!(instant::sanitize_name(""), None);
    // Length-capped at 32.
    let long = "a".repeat(64);
    assert_eq!(instant::sanitize_name(&long).unwrap().len(), 32);
    // Must not lead with a separator.
    assert_eq!(instant::sanitize_name("--x"), Some("x".into()));
}

#[test]
fn enabled_with_defaults_on_and_honors_kill_switch() {
    assert!(instant::enabled_with(&|_| None));
    assert!(!instant::enabled_with(&|k| {
        (k == "AINM_INSTANT_REGISTRATION").then(|| "0".to_string())
    }));
    assert!(!instant::enabled_with(&|k| {
        (k == "AINM_INSTANT_REGISTRATION").then(|| "false".to_string())
    }));
    assert!(instant::enabled_with(&|k| {
        (k == "AINM_INSTANT_REGISTRATION").then(|| "1".to_string())
    }));
}
