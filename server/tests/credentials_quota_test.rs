//! Integration tests for per-user credentials + quota (sub-project C).

use ainm_server::{credentials, quota};

/// Acquire a test pool or skip. Mirrors the helper used by the hosted-agent
/// integration tests (TEST_DATABASE_URL → AINM_DATABASE__URL → DATABASE_URL).
async fn pool_or_skip() -> Option<sqlx::PgPool> {
    let url = std::env::var("TEST_DATABASE_URL")
        .or_else(|_| std::env::var("AINM_DATABASE__URL"))
        .or_else(|_| std::env::var("DATABASE_URL"))
        .ok()?;
    let pool = sqlx::PgPool::connect(&url).await.ok()?;
    let migrations_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations");
    sqlx::migrate::Migrator::new(migrations_dir)
        .await
        .ok()?
        .run(&pool)
        .await
        .ok()?;
    Some(pool)
}

/// Create a throwaway active account, return (id, address).
async fn make_account(pool: &sqlx::PgPool, tag: &str) -> (uuid::Uuid, String) {
    let address = format!("c-{tag}-{:x}@ainm.local", uuid::Uuid::new_v4().as_u128());
    let id: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO accounts (username, domain, address, email_hash, email_verified, status) \
         VALUES ($1, 'ainm.local', $2, $3, true, 'active') RETURNING id",
    )
    .bind(format!("c-{tag}"))
    .bind(&address)
    .bind(format!("hash-{address}"))
    .fetch_one(pool)
    .await
    .unwrap();
    (id, address)
}

#[tokio::test]
async fn byo_key_stored_encrypted_and_decryptable() {
    let Some(pool) = pool_or_skip().await else {
        eprintln!("no DB; skipping");
        return;
    };
    let (account_id, _addr) = make_account(&pool, "byo").await;
    let cipher = credentials::CredCipher::new(b"test-master-secret");

    let (ct, nonce) = cipher.encrypt("sk-user-secret").unwrap();
    credentials::upsert_byo_key(&pool, account_id, &ct, &nonce, Some("gpt-4o"), None)
        .await
        .unwrap();

    let settings = credentials::get_settings(&pool, account_id)
        .await
        .unwrap()
        .unwrap();
    assert_ne!(
        settings.api_key_ciphertext.as_deref().unwrap(),
        b"sk-user-secret"
    );
    match credentials::resolve_credential(Some(&cipher), Some(&settings)) {
        credentials::Credential::ByoKey { api_key, model, .. } => {
            assert_eq!(api_key, "sk-user-secret");
            assert_eq!(model.as_deref(), Some("gpt-4o"));
        }
        _ => panic!("expected ByoKey"),
    }

    credentials::clear_byo_key(&pool, account_id).await.unwrap();
    let after = credentials::get_settings(&pool, account_id)
        .await
        .unwrap()
        .unwrap();
    assert!(after.api_key_ciphertext.is_none());
    assert!(matches!(
        credentials::resolve_credential(Some(&cipher), Some(&after)),
        credentials::Credential::ServerDefault
    ));
}

#[tokio::test]
async fn debit_accumulates_and_rolls_over_monthly() {
    let Some(pool) = pool_or_skip().await else {
        eprintln!("no DB; skipping");
        return;
    };
    let (account_id, addr) = make_account(&pool, "debit").await;

    let t1 = quota::debit(&pool, &addr, Some(account_id), "2026-06", 100)
        .await
        .unwrap();
    assert_eq!(t1, 100);
    let t2 = quota::debit(&pool, &addr, Some(account_id), "2026-06", 50)
        .await
        .unwrap();
    assert_eq!(t2, 150);
    assert_eq!(
        quota::current_usage(&pool, &addr, "2026-06").await.unwrap(),
        150
    );

    let t3 = quota::debit(&pool, &addr, Some(account_id), "2026-07", 10)
        .await
        .unwrap();
    assert_eq!(t3, 10);
    assert_eq!(
        quota::current_usage(&pool, &addr, "2026-06").await.unwrap(),
        150
    );
}

#[tokio::test]
async fn metered_client_blocks_over_budget_against_real_ledger() {
    use ainm_server::error::AppError;
    use ainm_server::llm::{
        AssistantTurn, ChatMessage, ChatResponse, ConversationMessage, ExtractionResult, LlmClient,
        MeteredLlmClient, QuotaStore, TokenUsage, ToolSchema,
    };

    let Some(pool) = pool_or_skip().await else {
        eprintln!("no DB; skipping");
        return;
    };
    let (account_id, addr) = make_account(&pool, "meter").await;

    quota::debit(&pool, &addr, Some(account_id), "2026-06", 1000)
        .await
        .unwrap();

    struct CallCountInner(std::sync::Mutex<u32>);
    #[async_trait::async_trait]
    impl LlmClient for CallCountInner {
        async fn extract(&self, _m: &[ConversationMessage]) -> Result<ExtractionResult, AppError> {
            Ok(ExtractionResult {
                nodes: vec![],
                edges: vec![],
            })
        }
        async fn chat_with_tools(
            &self,
            _m: &[ChatMessage],
            _t: &[ToolSchema],
        ) -> Result<ChatResponse, AppError> {
            *self.0.lock().unwrap() += 1;
            Ok(ChatResponse {
                turn: AssistantTurn::Text("hi".into()),
                usage: Some(TokenUsage {
                    prompt_tokens: 1,
                    completion_tokens: 1,
                    total_tokens: 2,
                }),
            })
        }
    }
    struct Store {
        pool: sqlx::PgPool,
        addr: String,
        account_id: uuid::Uuid,
    }
    #[async_trait::async_trait]
    impl QuotaStore for Store {
        async fn current_used(&self) -> i64 {
            quota::current_usage(&self.pool, &self.addr, "2026-06")
                .await
                .unwrap_or(0)
        }
        async fn debit(&self, t: i64) {
            let _ = quota::debit(&self.pool, &self.addr, Some(self.account_id), "2026-06", t).await;
        }
    }

    let inner = std::sync::Arc::new(CallCountInner(std::sync::Mutex::new(0)));
    let m = MeteredLlmClient {
        inner: inner.clone(),
        store: std::sync::Arc::new(Store {
            pool: pool.clone(),
            addr: addr.clone(),
            account_id,
        }),
        budget: 1000,
        enforce: true,
        missing_usage_fallback: 1000,
    };
    let r = m.chat_with_tools(&[], &[]).await.unwrap();
    match r.turn {
        AssistantTurn::Text(s) => assert!(s.to_lowercase().contains("limit")),
        _ => panic!("expected limit"),
    }
    assert_eq!(
        *inner.0.lock().unwrap(),
        0,
        "inner must not be called when over budget"
    );
}
