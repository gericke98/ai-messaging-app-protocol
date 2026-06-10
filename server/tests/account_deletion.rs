//! Account deletion (M7 / App Store guideline 5.1.1).
//! Requires PostgreSQL: set TEST_DATABASE_URL or runs against the default dev DB.
//!
//! Deletion contract: the account row and everything keyed to it (keys, escrow,
//! auth sessions, LLM settings — via FK CASCADE) plus the address-scoped personal
//! data (memory graph, grants, briefings, compilation passes, archival facts, the
//! caller's own blocks, usage meters, email verifications) are removed in one
//! transaction. Messages and content reports are retained deliberately: messages
//! belong to shared conversations, reports are the operator's moderation record.

use ainm_server::{account_deletion, accounts, blocks};
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

async fn count(pool: &PgPool, sql: &str, bind: &str) -> i64 {
    let (n,): (i64,) = sqlx::query_as(sql)
        .bind(bind)
        .fetch_one(pool)
        .await
        .unwrap();
    n
}

#[tokio::test]
async fn delete_account_removes_account_and_personal_data() {
    let Some(pool) = test_pool().await else {
        return;
    };
    let tag = format!("{:x}", Uuid::new_v4().as_fields().0);
    let username = format!("deluser{tag}");
    let email_hash = format!("delete-test:{tag}");
    let account =
        accounts::create_pending_account(&pool, &username, "test.ainm.local", &email_hash)
            .await
            .unwrap();
    let addr = account.address.clone();
    accounts::insert_account_key(&pool, account.id, &format!("delkey-{tag}"), Some("dev"))
        .await
        .unwrap();

    // Seed address-scoped personal data.
    sqlx::query(
        "INSERT INTO memory_nodes (user_addr, node_type, category, label) \
         VALUES ($1, 'semantic', 'identity', 'test node')",
    )
    .bind(&addr)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO memory_grants (user_addr, agent_addr) VALUES ($1, 'agent@x')")
        .bind(&addr)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO archival_facts (fact_id, participant_addr, key, value, stored_by) \
         VALUES ($1, $2, 'k', '\"v\"', $2)",
    )
    .bind(format!("fact-{tag}"))
    .bind(&addr)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO account_auth_tokens (account_id, token_hash, expires_at) \
         VALUES ($1, $2, now() + interval '1 day')",
    )
    .bind(account.id)
    .bind(tag.as_bytes())
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO llm_usage (requester_address, period, account_id, used_tokens) \
         VALUES ($1, '2026-06', $2, 10)",
    )
    .bind(&addr)
    .bind(account.id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO email_verifications (email_hash, code_hash, purpose, expires_at) \
         VALUES ($1, 'x', 'signup', now())",
    )
    .bind(&email_hash)
    .execute(&pool)
    .await
    .unwrap();

    // Blocks in both directions + a report BY the caller.
    let troll = format!("troll{tag}@test.ainm.local");
    let victim = format!("victim{tag}@test.ainm.local");
    blocks::block(&pool, &addr, &troll).await.unwrap(); // caller's own block → deleted
    blocks::block(&pool, &victim, &addr).await.unwrap(); // someone blocks caller → retained
    blocks::report(&pool, &addr, &troll, None, None, "spam")
        .await
        .unwrap(); // moderation record → retained

    account_deletion::delete_account(&pool, account.id)
        .await
        .unwrap();

    // The account and everything keyed to it are gone.
    assert!(accounts::find_account_by_address(&pool, &addr)
        .await
        .unwrap()
        .is_none());
    assert_eq!(
        count(&pool, "SELECT count(*) FROM account_keys ak JOIN accounts a ON a.id = ak.account_id WHERE a.address = $1", &addr).await,
        0
    );
    assert_eq!(
        count(
            &pool,
            "SELECT count(*) FROM memory_nodes WHERE user_addr = $1",
            &addr
        )
        .await,
        0
    );
    assert_eq!(
        count(
            &pool,
            "SELECT count(*) FROM memory_grants WHERE user_addr = $1",
            &addr
        )
        .await,
        0
    );
    assert_eq!(
        count(
            &pool,
            "SELECT count(*) FROM archival_facts WHERE participant_addr = $1",
            &addr
        )
        .await,
        0
    );
    assert_eq!(
        count(
            &pool,
            "SELECT count(*) FROM llm_usage WHERE requester_address = $1",
            &addr
        )
        .await,
        0
    );
    assert_eq!(
        count(
            &pool,
            "SELECT count(*) FROM email_verifications WHERE email_hash = $1",
            &email_hash
        )
        .await,
        0
    );
    // The caller's own blocks are gone; blocks protecting others from the
    // (now-deleted) address remain.
    assert!(blocks::list_blocks(&pool, &addr).await.unwrap().is_empty());
    assert_eq!(
        blocks::list_blocks(&pool, &victim).await.unwrap(),
        vec![addr.clone()]
    );
    // Reports the caller filed remain for moderation.
    assert_eq!(
        count(
            &pool,
            "SELECT count(*) FROM content_reports WHERE reporter_addr = $1",
            &addr
        )
        .await,
        1
    );
}

#[tokio::test]
async fn delete_account_unknown_id_is_noop() {
    let Some(pool) = test_pool().await else {
        return;
    };
    account_deletion::delete_account(&pool, Uuid::new_v4())
        .await
        .unwrap();
}
