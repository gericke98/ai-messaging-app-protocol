//! Integration tests for AINM compound memory operations.
//!
//! Requires PostgreSQL: set TEST_DATABASE_URL or runs against the default dev database.
//! Tests use unique user_addr values to avoid interference.

use ainm_demo::briefing::{self, ConsentLevel};
use ainm_demo::compiler::{self, CompileRequest};
use ainm_demo::graph;
use ainm_demo::llm::{
    CandidateEdge, CandidateNode, ConversationMessage, ExtractionResult, MockLlmClient,
};
use ainm_demo::repository;
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

// ── Test Helper ───────────────────────────────────────────────────────────

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
    let migrator = sqlx::migrate::Migrator::new(migrations_dir)
        .await
        .expect("Failed to load migrations");
    migrator.run(&pool).await.expect("Failed to run migrations");

    Some(pool)
}

fn unique_addr(prefix: &str) -> String {
    format!("{}_{:x}@test.ainm.local", prefix, Uuid::new_v4().as_fields().0)
}

// ── Graph: Node CRUD ──────────────────────────────────────────────────────

#[tokio::test]
async fn test_insert_and_query_node() {
    let Some(pool) = test_pool().await else { return };
    let user = unique_addr("node_crud");

    let node_id = graph::insert_node(
        &pool,
        &user,
        "semantic",
        "fact",
        "Prefers dark mode",
        &serde_json::json!({"theme": "dark"}),
        0.9,
        &serde_json::json!({"contributed_by": "test"}),
        &serde_json::json!(["ui", "preference"]),
    )
    .await
    .expect("insert_node failed");

    let nodes = graph::query_nodes(&pool, &user, None, None, None, "current", 0.0, 10)
        .await
        .expect("query_nodes failed");

    assert_eq!(nodes.len(), 1);
    assert_eq!(nodes[0].id, node_id);
    assert_eq!(nodes[0].label, "Prefers dark mode");
    assert_eq!(nodes[0].category, "fact");
    assert!((nodes[0].confidence - 0.9).abs() < 0.01);
}

#[tokio::test]
async fn test_query_nodes_text_filter() {
    let Some(pool) = test_pool().await else { return };
    let user = unique_addr("text_filter");

    graph::insert_node(
        &pool, &user, "semantic", "skill", "TypeScript expert",
        &serde_json::json!({}), 0.85,
        &serde_json::json!({"contributed_by": "test"}),
        &serde_json::json!([]),
    ).await.unwrap();

    graph::insert_node(
        &pool, &user, "semantic", "preference", "Prefers Vim keybindings",
        &serde_json::json!({}), 0.7,
        &serde_json::json!({"contributed_by": "test"}),
        &serde_json::json!([]),
    ).await.unwrap();

    let results = graph::query_nodes(&pool, &user, Some("TypeScript"), None, None, "current", 0.0, 10)
        .await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].label, "TypeScript expert");
}

#[tokio::test]
async fn test_query_nodes_min_confidence() {
    let Some(pool) = test_pool().await else { return };
    let user = unique_addr("min_conf");

    graph::insert_node(&pool, &user, "semantic", "fact", "High confidence", &serde_json::json!({}), 0.95, &serde_json::json!({}), &serde_json::json!([])).await.unwrap();
    graph::insert_node(&pool, &user, "semantic", "fact", "Low confidence", &serde_json::json!({}), 0.3, &serde_json::json!({}), &serde_json::json!([])).await.unwrap();

    let results = graph::query_nodes(&pool, &user, None, None, None, "current", 0.5, 10).await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].label, "High confidence");
}

// ── Graph: Edges ──────────────────────────────────────────────────────────

#[tokio::test]
async fn test_insert_and_query_edges() {
    let Some(pool) = test_pool().await else { return };
    let user = unique_addr("edges");

    let n1 = graph::insert_node(&pool, &user, "semantic", "skill", "Rust", &serde_json::json!({}), 0.9, &serde_json::json!({}), &serde_json::json!([])).await.unwrap();
    let n2 = graph::insert_node(&pool, &user, "semantic", "tool", "Axum", &serde_json::json!({}), 0.85, &serde_json::json!({}), &serde_json::json!([])).await.unwrap();

    let edge_id = graph::insert_edge(
        &pool, &user, n1, n2, "uses", 1.0, 0.9,
        &serde_json::json!({}), &serde_json::json!({}),
    ).await.expect("insert_edge failed");

    let edges = graph::get_edges_for_nodes(&pool, &user, &[n1, n2]).await.unwrap();
    assert_eq!(edges.len(), 1);
    assert_eq!(edges[0].id, edge_id);
    assert_eq!(edges[0].relationship, "uses");
    assert_eq!(edges[0].source_id, n1);
    assert_eq!(edges[0].target_id, n2);
}

// ── Graph: Traversal ──────────────────────────────────────────────────────

#[tokio::test]
async fn test_graph_traversal() {
    let Some(pool) = test_pool().await else { return };
    let user = unique_addr("traverse");

    // Build: A --uses--> B --related_to--> C
    let a = graph::insert_node(&pool, &user, "semantic", "skill", "Node A", &serde_json::json!({}), 0.9, &serde_json::json!({}), &serde_json::json!([])).await.unwrap();
    let b = graph::insert_node(&pool, &user, "semantic", "tool", "Node B", &serde_json::json!({}), 0.85, &serde_json::json!({}), &serde_json::json!([])).await.unwrap();
    let c = graph::insert_node(&pool, &user, "semantic", "fact", "Node C", &serde_json::json!({}), 0.8, &serde_json::json!({}), &serde_json::json!([])).await.unwrap();

    graph::insert_edge(&pool, &user, a, b, "uses", 1.0, 0.9, &serde_json::json!({}), &serde_json::json!({})).await.unwrap();
    graph::insert_edge(&pool, &user, b, c, "related_to", 1.0, 0.85, &serde_json::json!({}), &serde_json::json!({})).await.unwrap();

    // Traverse from A, depth 2 — should reach B and C
    let (nodes, edges) = graph::traverse(&pool, &user, a, 2, "outbound", None).await.unwrap();
    assert!(nodes.len() >= 2, "Should reach at least B and C, got {}", nodes.len());

    // Traverse from A, depth 1 — should reach only B
    let (nodes_d1, _) = graph::traverse(&pool, &user, a, 1, "outbound", None).await.unwrap();
    assert_eq!(nodes_d1.len(), 1, "Depth 1 should reach only B");
}

// ── Graph: Lint ───────────────────────────────────────────────────────────

#[tokio::test]
async fn test_find_orphans() {
    let Some(pool) = test_pool().await else { return };
    let user = unique_addr("orphans");

    // Create a connected pair and an orphan
    let n1 = graph::insert_node(&pool, &user, "semantic", "fact", "Connected A", &serde_json::json!({}), 0.9, &serde_json::json!({}), &serde_json::json!([])).await.unwrap();
    let n2 = graph::insert_node(&pool, &user, "semantic", "fact", "Connected B", &serde_json::json!({}), 0.9, &serde_json::json!({}), &serde_json::json!([])).await.unwrap();
    let orphan = graph::insert_node(&pool, &user, "semantic", "fact", "Orphan Node", &serde_json::json!({}), 0.9, &serde_json::json!({}), &serde_json::json!([])).await.unwrap();

    graph::insert_edge(&pool, &user, n1, n2, "related_to", 1.0, 0.9, &serde_json::json!({}), &serde_json::json!({})).await.unwrap();

    let orphans = graph::find_orphans(&pool, &user).await.unwrap();
    assert!(orphans.contains(&orphan), "Should detect orphan node");
    assert!(!orphans.contains(&n1), "Connected nodes should not be orphans");
}

#[tokio::test]
async fn test_graph_stats() {
    let Some(pool) = test_pool().await else { return };
    let user = unique_addr("stats");

    let n1 = graph::insert_node(&pool, &user, "semantic", "fact", "Stat A", &serde_json::json!({}), 0.9, &serde_json::json!({}), &serde_json::json!([])).await.unwrap();
    let n2 = graph::insert_node(&pool, &user, "semantic", "fact", "Stat B", &serde_json::json!({}), 0.9, &serde_json::json!({}), &serde_json::json!([])).await.unwrap();
    graph::insert_edge(&pool, &user, n1, n2, "related_to", 1.0, 0.9, &serde_json::json!({}), &serde_json::json!({})).await.unwrap();

    let (nodes, edges) = graph::graph_stats(&pool, &user).await.unwrap();
    assert_eq!(nodes, 2);
    assert_eq!(edges, 1);
}

#[tokio::test]
async fn test_export_graph() {
    let Some(pool) = test_pool().await else { return };
    let user = unique_addr("export");

    let n1 = graph::insert_node(&pool, &user, "semantic", "fact", "Export A", &serde_json::json!({}), 0.9, &serde_json::json!({}), &serde_json::json!([])).await.unwrap();
    let n2 = graph::insert_node(&pool, &user, "semantic", "fact", "Export B", &serde_json::json!({}), 0.85, &serde_json::json!({}), &serde_json::json!([])).await.unwrap();
    graph::insert_edge(&pool, &user, n1, n2, "supports", 1.0, 0.9, &serde_json::json!({}), &serde_json::json!({})).await.unwrap();

    let (nodes, edges) = graph::export_graph(&pool, &user, false).await.unwrap();
    assert_eq!(nodes.len(), 2);
    assert_eq!(edges.len(), 1);
}

// ── Graph: Supersession ───────────────────────────────────────────────────

#[tokio::test]
async fn test_supersede_node() {
    let Some(pool) = test_pool().await else { return };
    let user = unique_addr("supersede");

    let old = graph::insert_node(&pool, &user, "semantic", "fact", "Likes tea", &serde_json::json!({}), 0.9, &serde_json::json!({}), &serde_json::json!([])).await.unwrap();
    let new = graph::insert_node(&pool, &user, "semantic", "fact", "Likes coffee", &serde_json::json!({}), 0.95, &serde_json::json!({}), &serde_json::json!([])).await.unwrap();

    graph::supersede_node(&pool, old, new).await.unwrap();

    // Query current nodes — should only return the new one
    let current = graph::query_nodes(&pool, &user, None, None, None, "current", 0.0, 10).await.unwrap();
    let labels: Vec<&str> = current.iter().map(|n| n.label.as_str()).collect();
    assert!(labels.contains(&"Likes coffee"));
    assert!(!labels.contains(&"Likes tea"));

    // Query all (including superseded)
    let all = graph::query_nodes(&pool, &user, None, None, None, "all", 0.0, 10).await.unwrap();
    assert!(all.len() >= 2);
}

// ── Repository: Archival Facts ────────────────────────────────────────────

#[tokio::test]
async fn test_archival_fact_crud() {
    let Some(pool) = test_pool().await else { return };
    let user = unique_addr("facts");
    let fact_id = Uuid::new_v4().to_string();

    repository::store_fact(
        &pool, &fact_id, &user, Some("test-thread"),
        "favorite_color", &serde_json::json!("blue"),
        &serde_json::json!(["personal"]), &user,
    ).await.expect("store_fact failed");

    let facts = repository::query_facts(&pool, &user, Some("favorite"), 10).await.unwrap();
    assert_eq!(facts.len(), 1);
    assert_eq!(facts[0].key, "favorite_color");

    repository::evict_fact(&pool, &user, "favorite_color").await.unwrap();
    let after = repository::query_facts(&pool, &user, Some("favorite"), 10).await.unwrap();
    assert_eq!(after.len(), 0);
}

// ── Briefing: Scope Resolution ────────────────────────────────────────────

#[tokio::test]
async fn test_resolve_scopes_default() {
    let Some(pool) = test_pool().await else { return };
    let user = unique_addr("scopes_def");
    let agent = unique_addr("agent_def");

    let (scopes, level) = briefing::resolve_scopes(&pool, &user, &agent, ConsentLevel::Standard)
        .await.unwrap();

    assert_eq!(level, ConsentLevel::Standard);
    assert!(scopes.contains(&"identity".to_string()));
    assert!(scopes.contains(&"preferences".to_string()));
    assert!(!scopes.contains(&"history".to_string()));
}

#[tokio::test]
async fn test_grant_and_resolve_scopes() {
    let Some(pool) = test_pool().await else { return };
    let user = unique_addr("grant");
    let agent = unique_addr("agent_grant");

    briefing::grant_scopes(
        &pool, &user, &agent,
        &["identity".to_string(), "history".to_string()],
        ConsentLevel::Full,
    ).await.unwrap();

    let (scopes, level) = briefing::resolve_scopes(&pool, &user, &agent, ConsentLevel::Minimal)
        .await.unwrap();

    assert_eq!(level, ConsentLevel::Full);
    assert!(scopes.contains(&"identity".to_string()));
    assert!(scopes.contains(&"history".to_string()));
}

#[tokio::test]
async fn test_revoke_scopes() {
    let Some(pool) = test_pool().await else { return };
    let user = unique_addr("revoke");
    let agent = unique_addr("agent_rev");

    briefing::grant_scopes(
        &pool, &user, &agent,
        &["identity".to_string(), "history".to_string(), "patterns".to_string()],
        ConsentLevel::Full,
    ).await.unwrap();

    briefing::revoke_scopes(&pool, &user, &agent, &["history".to_string()]).await.unwrap();

    let (scopes, _) = briefing::resolve_scopes(&pool, &user, &agent, ConsentLevel::Minimal)
        .await.unwrap();
    assert!(scopes.contains(&"identity".to_string()));
    assert!(!scopes.contains(&"history".to_string()));
    assert!(scopes.contains(&"patterns".to_string()));
}

// ── Briefing: Generation ──────────────────────────────────────────────────

#[tokio::test]
async fn test_generate_briefing() {
    let Some(pool) = test_pool().await else { return };
    let user = unique_addr("briefing");
    let agent = unique_addr("agent_brief");

    // Seed some nodes
    graph::insert_node(&pool, &user, "semantic", "fact", "Software engineer", &serde_json::json!({"role": "engineer"}), 0.95, &serde_json::json!({}), &serde_json::json!([])).await.unwrap();
    graph::insert_node(&pool, &user, "semantic", "preference", "Prefers dark mode", &serde_json::json!({}), 0.8, &serde_json::json!({}), &serde_json::json!([])).await.unwrap();
    graph::insert_node(&pool, &user, "semantic", "skill", "Expert in Rust", &serde_json::json!({}), 0.9, &serde_json::json!({}), &serde_json::json!([])).await.unwrap();

    let briefing = briefing::generate_briefing(&pool, &user, &agent, "thread-123", ConsentLevel::Standard)
        .await.unwrap();

    assert!(briefing.get("briefing_id").is_some());
    assert_eq!(briefing.get("thread_id").unwrap().as_str().unwrap(), "thread-123");
    assert!(briefing.get("sections").is_some());

    let sections = briefing.get("sections").unwrap().as_object().unwrap();
    assert!(sections.contains_key("identity") || sections.contains_key("preferences") || sections.contains_key("expertise"));
}

#[tokio::test]
async fn test_briefing_minimal_consent() {
    let Some(pool) = test_pool().await else { return };
    let user = unique_addr("brief_min");
    let agent = unique_addr("agent_min");

    graph::insert_node(&pool, &user, "semantic", "fact", "Developer", &serde_json::json!({}), 0.9, &serde_json::json!({}), &serde_json::json!([])).await.unwrap();
    graph::insert_node(&pool, &user, "semantic", "skill", "Python expert", &serde_json::json!({}), 0.9, &serde_json::json!({}), &serde_json::json!([])).await.unwrap();

    let briefing = briefing::generate_briefing(&pool, &user, &agent, "thread-min", ConsentLevel::Minimal)
        .await.unwrap();

    let sections = briefing.get("sections").unwrap().as_object().unwrap();
    // Minimal consent only shares identity (fact category) — not skills
    assert!(!sections.contains_key("expertise"));
}

// ── Compiler: Full Pipeline ───────────────────────────────────────────────

#[tokio::test]
async fn test_compilation_with_mock_llm() {
    let Some(pool) = test_pool().await else { return };
    let user = unique_addr("compile");

    let mock = MockLlmClient::new(vec![ExtractionResult {
        nodes: vec![
            CandidateNode {
                node_type: "semantic".to_string(),
                category: "preference".to_string(),
                label: "Likes Rust".to_string(),
                confidence: 0.9,
                properties: serde_json::json!({"language": "rust"}),
                tags: vec!["programming".to_string()],
            },
            CandidateNode {
                node_type: "semantic".to_string(),
                category: "tool".to_string(),
                label: "Uses VS Code".to_string(),
                confidence: 0.85,
                properties: serde_json::json!({}),
                tags: vec![],
            },
        ],
        edges: vec![CandidateEdge {
            source_label: "Likes Rust".to_string(),
            target_label: "Uses VS Code".to_string(),
            relationship: "uses_with".to_string(),
            confidence: 0.8,
        }],
    }]);

    let req = CompileRequest {
        user_addr: user.clone(),
        thread_id: "compile-thread".to_string(),
        messages: vec![
            ConversationMessage { from: user.clone(), content: "I really enjoy writing Rust code".to_string(), timestamp: "2026-01-01T00:00:00Z".to_string() },
            ConversationMessage { from: "agent@test".to_string(), content: "That's great! Rust is excellent for systems programming".to_string(), timestamp: "2026-01-01T00:01:00Z".to_string() },
        ],
    };

    let result = compiler::run_compilation(pool.clone(), Arc::new(mock), req)
        .await.expect("compilation failed");

    assert_eq!(result.nodes_created, 2);
    assert_eq!(result.edges_created, 1);

    // Verify nodes exist in the graph
    let nodes = graph::query_nodes(&pool, &user, None, None, None, "current", 0.0, 10).await.unwrap();
    let labels: Vec<&str> = nodes.iter().map(|n| n.label.as_str()).collect();
    assert!(labels.contains(&"Likes Rust"));
    assert!(labels.contains(&"Uses VS Code"));

    // Verify edge exists
    let node_ids: Vec<uuid::Uuid> = nodes.iter().map(|n| n.id).collect();
    let edges = graph::get_edges_for_nodes(&pool, &user, &node_ids).await.unwrap();
    assert_eq!(edges.len(), 1);
    assert_eq!(edges[0].relationship, "uses_with");
}

#[tokio::test]
async fn test_compilation_skips_trivial() {
    let Some(pool) = test_pool().await else { return };
    let user = unique_addr("trivial");

    let mock = MockLlmClient::empty();

    let req = CompileRequest {
        user_addr: user,
        thread_id: "trivial-thread".to_string(),
        messages: vec![
            ConversationMessage { from: "user@test".to_string(), content: "hi".to_string(), timestamp: "2026-01-01T00:00:00Z".to_string() },
            ConversationMessage { from: "agent@test".to_string(), content: "ok".to_string(), timestamp: "2026-01-01T00:01:00Z".to_string() },
        ],
    };

    let result = compiler::run_compilation(pool, Arc::new(mock), req).await.unwrap();
    assert_eq!(result.nodes_created, 0);
    assert_eq!(result.edges_created, 0);
}

#[tokio::test]
async fn test_compilation_dedup_updates_confidence() {
    let Some(pool) = test_pool().await else { return };
    let user = unique_addr("dedup");

    // First compilation — creates "Likes Rust" at 0.7
    let mock1 = MockLlmClient::new(vec![ExtractionResult {
        nodes: vec![CandidateNode {
            node_type: "semantic".to_string(),
            category: "preference".to_string(),
            label: "Likes Rust".to_string(),
            confidence: 0.7,
            properties: serde_json::json!({}),
            tags: vec![],
        }],
        edges: vec![],
    }]);

    let req1 = CompileRequest {
        user_addr: user.clone(),
        thread_id: "dedup-thread".to_string(),
        messages: vec![
            ConversationMessage { from: user.clone(), content: "I like Rust programming".to_string(), timestamp: "2026-01-01T00:00:00Z".to_string() },
            ConversationMessage { from: "agent@test".to_string(), content: "Noted, you prefer Rust".to_string(), timestamp: "2026-01-01T00:01:00Z".to_string() },
        ],
    };

    let r1 = compiler::run_compilation(pool.clone(), Arc::new(mock1), req1).await.unwrap();
    assert_eq!(r1.nodes_created, 1);

    // Second compilation — same label at 0.9 should update confidence, not create new
    let mock2 = MockLlmClient::new(vec![ExtractionResult {
        nodes: vec![CandidateNode {
            node_type: "semantic".to_string(),
            category: "preference".to_string(),
            label: "Likes Rust".to_string(),
            confidence: 0.9,
            properties: serde_json::json!({}),
            tags: vec![],
        }],
        edges: vec![],
    }]);

    let req2 = CompileRequest {
        user_addr: user.clone(),
        thread_id: "dedup-thread".to_string(),
        messages: vec![
            ConversationMessage { from: user.clone(), content: "I really love Rust!".to_string(), timestamp: "2026-01-02T00:00:00Z".to_string() },
            ConversationMessage { from: "agent@test".to_string(), content: "Your Rust enthusiasm is clear!".to_string(), timestamp: "2026-01-02T00:01:00Z".to_string() },
        ],
    };

    let r2 = compiler::run_compilation(pool.clone(), Arc::new(mock2), req2).await.unwrap();
    assert_eq!(r2.nodes_created, 0, "Should not create duplicate node");
    assert_eq!(r2.nodes_updated, 1, "Should update existing node confidence");

    // Verify updated confidence
    let nodes = graph::query_nodes(&pool, &user, Some("Likes Rust"), None, None, "current", 0.0, 10).await.unwrap();
    assert_eq!(nodes.len(), 1);
    assert!(nodes[0].confidence >= 0.9, "Confidence should be updated to >= 0.9, got {}", nodes[0].confidence);
}

// ── Briefing: Query-conditioned + Peer Card ───────────────────────────────

#[tokio::test]
async fn briefing_is_query_conditioned_and_has_peer_card() {
    let Some(pool) = test_pool().await else { return; };
    let user = unique_addr("bqc_user");
    let thread = unique_addr("bqc_thread");
    sqlx::query("INSERT INTO threads (id, origin_domain) VALUES ($1, $2) ON CONFLICT DO NOTHING")
        .bind(&thread).bind("test.local").execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO messages (thread_id, sequence, from_addr, content) VALUES ($1,1,$2,$3)")
        .bind(&thread).bind(&user).bind(serde_json::json!({"text": "help with my rust backend"}))
        .execute(&pool).await.unwrap();
    let prov = serde_json::json!({});
    let tags = serde_json::json!([]);
    ainm_demo::graph::insert_node(&pool, &user, "semantic", "preferences",
        "Prefers Rust for backend services", &serde_json::json!({}), 0.9, &prov, &tags).await.unwrap();
    ainm_demo::graph::insert_node(&pool, &user, "semantic", "identity",
        "Name is Santiago", &serde_json::json!({}), 0.95, &prov, &tags).await.unwrap();
    ainm_demo::graph::insert_node(&pool, &user, "semantic", "preferences",
        "Enjoys hiking on weekends", &serde_json::json!({}), 0.9, &prov, &tags).await.unwrap();

    let briefing = ainm_demo::briefing::generate_briefing(
        &pool, &user, "agent@x", &thread, ainm_demo::briefing::ConsentLevel::Standard,
    ).await.unwrap();

    let card = briefing.get("peer_card").and_then(|v| v.as_str()).unwrap_or("");
    assert!(card.contains("Santiago"), "peer_card should contain identity: {card}");
    let blob = serde_json::to_string(&briefing).unwrap();
    assert!(blob.contains("Prefers Rust"), "relevant fact should be injected");
    assert!(briefing.get("char_budget_used").is_some());
}

// ── Graph: Thread Message Texts ───────────────────────────────────────────

#[tokio::test]
async fn fetch_recent_thread_texts_returns_newest_first() {
    let Some(pool) = test_pool().await else { return; };
    let thread = unique_addr("thread_texts");
    sqlx::query("INSERT INTO threads (id, origin_domain) VALUES ($1, $2) ON CONFLICT DO NOTHING")
        .bind(&thread).bind("test.ainm.local").execute(&pool).await.unwrap();
    for (seq, txt) in [(1i64, "hello rust"), (2, "postgres migration")] {
        sqlx::query("INSERT INTO messages (thread_id, sequence, from_addr, content) VALUES ($1,$2,$3,$4)")
            .bind(&thread).bind(seq).bind("u@a")
            .bind(serde_json::json!({"text": txt}))
            .execute(&pool).await.unwrap();
    }
    let texts = ainm_demo::graph::fetch_recent_thread_texts(&pool, &thread, 10).await.unwrap();
    assert_eq!(texts.len(), 2);
    assert_eq!(texts[0], "postgres migration");
    assert!(texts.contains(&"hello rust".to_string()));
}
