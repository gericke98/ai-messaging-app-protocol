//! Integration tests for account auth sessions (live Postgres; skips without a DB).

use ainm_server::auth_session;
use chrono::{Duration, Utc};

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
    let username = format!("as-{tag}-{suffix}");
    let address = format!("{username}@ainm.local");
    sqlx::query_scalar(
        "INSERT INTO accounts (username, domain, address, email_hash, email_verified, status) \
         VALUES ($1, 'ainm.local', $2, $3, true, 'active') RETURNING id",
    )
    .bind(&username).bind(&address).bind(format!("hash-{address}"))
    .fetch_one(pool).await.unwrap()
}

#[tokio::test]
async fn mint_use_list_then_revoke_cycle() {
    let Some(pool) = pool_or_skip().await else { eprintln!("no DB; skipping"); return; };
    let account_id = make_account(&pool, "cycle").await;
    let now = Utc::now();
    let (token, hash) = auth_session::generate_token();
    let expires = auth_session::next_expiry(now, now);
    let session_id = auth_session::insert(&pool, account_id, &hash, Some("iPhone"), now, expires).await.unwrap();

    let caller = auth_session::authenticate(&pool, &token, now).await.unwrap().expect("active");
    assert_eq!(caller.account_id, account_id);
    assert_eq!(caller.token_hash, hash);

    let sessions = auth_session::list_active(&pool, account_id, now).await.unwrap();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].id, session_id);
    assert_eq!(sessions[0].device_label.as_deref(), Some("iPhone"));

    assert!(auth_session::revoke_one(&pool, account_id, session_id, now).await.unwrap());
    assert!(auth_session::authenticate(&pool, &token, now).await.unwrap().is_none());
}

#[tokio::test]
async fn expired_session_is_rejected() {
    let Some(pool) = pool_or_skip().await else { eprintln!("no DB; skipping"); return; };
    let account_id = make_account(&pool, "expired").await;
    let now = Utc::now();
    let (token, hash) = auth_session::generate_token();
    let past = now - Duration::days(1);
    auth_session::insert(&pool, account_id, &hash, None, now - Duration::days(100), past).await.unwrap();
    assert!(auth_session::authenticate(&pool, &token, now).await.unwrap().is_none());
}

#[tokio::test]
async fn revoke_others_keeps_current() {
    let Some(pool) = pool_or_skip().await else { eprintln!("no DB; skipping"); return; };
    let account_id = make_account(&pool, "others").await;
    let now = Utc::now();
    let (tok_a, hash_a) = auth_session::generate_token();
    let (tok_b, hash_b) = auth_session::generate_token();
    let exp = auth_session::next_expiry(now, now);
    auth_session::insert(&pool, account_id, &hash_a, Some("A"), now, exp).await.unwrap();
    auth_session::insert(&pool, account_id, &hash_b, Some("B"), now, exp).await.unwrap();

    let revoked = auth_session::revoke_others(&pool, account_id, &hash_a, now).await.unwrap();
    assert_eq!(revoked, 1);
    assert!(auth_session::authenticate(&pool, &tok_a, now).await.unwrap().is_some());
    assert!(auth_session::authenticate(&pool, &tok_b, now).await.unwrap().is_none());
}

#[tokio::test]
async fn revoke_one_is_account_scoped() {
    let Some(pool) = pool_or_skip().await else { eprintln!("no DB; skipping"); return; };
    let victim = make_account(&pool, "victim").await;
    let attacker = make_account(&pool, "attacker").await;
    let now = Utc::now();
    let (_tok, hash) = auth_session::generate_token();
    let exp = auth_session::next_expiry(now, now);
    let sid = auth_session::insert(&pool, victim, &hash, None, now, exp).await.unwrap();
    assert!(!auth_session::revoke_one(&pool, attacker, sid, now).await.unwrap());
}
