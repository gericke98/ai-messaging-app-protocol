//! Push notification scaffolding (M4): token storage, APNs settings, payload,
//! and the ES256 provider-token JWT. The actual APNs HTTP call needs Apple
//! credentials + a physical device and is exercised at submission time.
//! DB tests require PostgreSQL (TEST_DATABASE_URL or the default dev DB).

use ainm_server::{accounts, push};
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

// --- Pure: settings ---

#[test]
fn apns_settings_require_all_four_vars() {
    let full = |k: &str| -> Option<String> {
        match k {
            "AINM_APNS__KEY_PATH" => Some("/keys/AuthKey_ABC123.p8".into()),
            "AINM_APNS__KEY_ID" => Some("ABC123".into()),
            "AINM_APNS__TEAM_ID" => Some("TEAM456".into()),
            "AINM_APNS__TOPIC" => Some("com.ainm.messenger".into()),
            _ => None,
        }
    };
    let s = push::settings_with(&full).expect("all vars set → Some");
    assert_eq!(s.key_id, "ABC123");
    assert_eq!(s.team_id, "TEAM456");
    assert_eq!(s.topic, "com.ainm.messenger");
    assert!(!s.sandbox, "sandbox defaults off");
    assert_eq!(s.host(), "https://api.push.apple.com");

    // Any missing var → None (push disabled, server unaffected).
    let partial = |k: &str| -> Option<String> {
        match k {
            "AINM_APNS__KEY_ID" => Some("ABC123".into()),
            _ => None,
        }
    };
    assert!(push::settings_with(&partial).is_none());

    // Sandbox flag flips the host.
    let sandboxed = |k: &str| -> Option<String> {
        if k == "AINM_APNS__SANDBOX" {
            Some("1".into())
        } else {
            full(k)
        }
    };
    let s = push::settings_with(&sandboxed).unwrap();
    assert!(s.sandbox);
    assert_eq!(s.host(), "https://api.sandbox.push.apple.com");
}

// --- Pure: payload ---

#[test]
fn apns_payload_carries_alert_and_thread_deep_link() {
    let p = push::apns_payload("alice", "hey, are you around?", "thread-42");
    assert_eq!(p["aps"]["alert"]["title"], "alice");
    assert_eq!(p["aps"]["alert"]["body"], "hey, are you around?");
    assert_eq!(p["aps"]["thread-id"], "thread-42");
    assert_eq!(p["aps"]["sound"], "default");
    // Custom key the client uses to deep-link on tap.
    assert_eq!(p["thread_id"], "thread-42");
}

#[test]
fn apns_payload_truncates_long_previews() {
    let long = "x".repeat(500);
    let p = push::apns_payload("bob", &long, "t");
    let body = p["aps"]["alert"]["body"].as_str().unwrap();
    assert!(
        body.chars().count() <= 140,
        "preview capped, got {}",
        body.len()
    );
}

// --- Pure: ES256 provider token ---

#[test]
fn provider_jwt_signs_and_verifies_with_es256() {
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use base64::Engine as _;

    // Generate a P-256 key like Apple's .p8 (PKCS#8), wrapped in PEM.
    let rng = ring::rand::SystemRandom::new();
    let pkcs8 = ring::signature::EcdsaKeyPair::generate_pkcs8(
        &ring::signature::ECDSA_P256_SHA256_FIXED_SIGNING,
        &rng,
    )
    .unwrap();
    let pem = format!(
        "-----BEGIN PRIVATE KEY-----\n{}\n-----END PRIVATE KEY-----\n",
        base64::engine::general_purpose::STANDARD.encode(pkcs8.as_ref())
    );

    let jwt = push::provider_jwt(&pem, "KEYID1", "TEAMID1", 1_780_000_000).unwrap();
    let parts: Vec<&str> = jwt.split('.').collect();
    assert_eq!(parts.len(), 3, "header.claims.signature");

    let header: serde_json::Value =
        serde_json::from_slice(&URL_SAFE_NO_PAD.decode(parts[0]).unwrap()).unwrap();
    assert_eq!(header["alg"], "ES256");
    assert_eq!(header["kid"], "KEYID1");
    let claims: serde_json::Value =
        serde_json::from_slice(&URL_SAFE_NO_PAD.decode(parts[1]).unwrap()).unwrap();
    assert_eq!(claims["iss"], "TEAMID1");
    assert_eq!(claims["iat"], 1_780_000_000);

    // The signature verifies against the key's public half.
    let key_pair = ring::signature::EcdsaKeyPair::from_pkcs8(
        &ring::signature::ECDSA_P256_SHA256_FIXED_SIGNING,
        pkcs8.as_ref(),
        &rng,
    )
    .unwrap();
    use ring::signature::KeyPair as _;
    let public = ring::signature::UnparsedPublicKey::new(
        &ring::signature::ECDSA_P256_SHA256_FIXED,
        key_pair.public_key().as_ref(),
    );
    let signed_input = format!("{}.{}", parts[0], parts[1]);
    let sig = URL_SAFE_NO_PAD.decode(parts[2]).unwrap();
    public.verify(signed_input.as_bytes(), &sig).unwrap();
}

// --- DB: token registration ---

#[tokio::test]
async fn register_list_remove_push_tokens() {
    let Some(pool) = test_pool().await else {
        return;
    };
    let tag = format!("{:x}", Uuid::new_v4().as_fields().0);
    let account = accounts::create_pending_account(
        &pool,
        &format!("pushuser{tag}"),
        "test.ainm.local",
        &format!("push-test:{tag}"),
    )
    .await
    .unwrap();

    let token_a = format!("apns-token-a-{tag}");
    let token_b = format!("apns-token-b-{tag}");
    push::register_token(&pool, account.id, &token_a, "ios")
        .await
        .unwrap();
    push::register_token(&pool, account.id, &token_a, "ios")
        .await
        .unwrap(); // idempotent upsert
    push::register_token(&pool, account.id, &token_b, "ios")
        .await
        .unwrap();

    let mut tokens = push::tokens_for_address(&pool, &account.address)
        .await
        .unwrap();
    tokens.sort();
    assert_eq!(tokens, vec![token_a.clone(), token_b.clone()]);

    push::remove_token(&pool, account.id, &token_a)
        .await
        .unwrap();
    assert_eq!(
        push::tokens_for_address(&pool, &account.address)
            .await
            .unwrap(),
        vec![token_b.clone()]
    );

    // A token re-registered by a DIFFERENT account moves to it (device changed owner).
    let other = accounts::create_pending_account(
        &pool,
        &format!("pushother{tag}"),
        "test.ainm.local",
        &format!("push-other:{tag}"),
    )
    .await
    .unwrap();
    push::register_token(&pool, other.id, &token_b, "ios")
        .await
        .unwrap();
    assert!(push::tokens_for_address(&pool, &account.address)
        .await
        .unwrap()
        .is_empty());
    assert_eq!(
        push::tokens_for_address(&pool, &other.address)
            .await
            .unwrap(),
        vec![token_b]
    );
}
