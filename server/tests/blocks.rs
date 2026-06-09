//! Block/report storage + delivery filtering (M7 / App Store guideline 1.2).
//! Requires PostgreSQL: set TEST_DATABASE_URL or runs against the default dev DB.

use ainm_server::blocks;
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

fn addr(prefix: &str) -> String {
    format!("{prefix}{:x}@test.ainm.local", Uuid::new_v4().as_fields().0)
}

#[tokio::test]
async fn block_unblock_roundtrip_and_idempotent_block() {
    let Some(pool) = test_pool().await else {
        return;
    };
    let (alice, troll) = (addr("alice"), addr("troll"));

    blocks::block(&pool, &alice, &troll).await.unwrap();
    blocks::block(&pool, &alice, &troll).await.unwrap(); // idempotent
    assert_eq!(
        blocks::list_blocks(&pool, &alice).await.unwrap(),
        vec![troll.clone()]
    );
    assert!(blocks::blockers_of(&pool, &troll)
        .await
        .unwrap()
        .contains(&alice));

    blocks::unblock(&pool, &alice, &troll).await.unwrap();
    assert!(blocks::list_blocks(&pool, &alice).await.unwrap().is_empty());
    assert!(!blocks::blockers_of(&pool, &troll)
        .await
        .unwrap()
        .contains(&alice));
}

#[tokio::test]
async fn report_is_persisted_open() {
    let Some(pool) = test_pool().await else {
        return;
    };
    let (reporter, subject) = (addr("rep"), addr("subj"));
    let id = blocks::report(
        &pool,
        &reporter,
        &subject,
        Some("thread-9"),
        Some("msg-3"),
        "harassment in thread",
    )
    .await
    .unwrap();

    let (status, reason): (String, String) =
        sqlx::query_as("SELECT status, reason FROM content_reports WHERE id = $1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(status, "open");
    assert_eq!(reason, "harassment in thread");
}

// NOTE: the HumanRegistry::send_to_thread_except filtering test lives as a unit
// test in server.rs — the `server` module is part of the binary's module tree,
// not the library.
