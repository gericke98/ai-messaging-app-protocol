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
    // Dynamic sections use the node's actual category as the key.
    assert!(!sections.is_empty(), "sections should not be empty when facts exist: {sections:?}");
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
    ainm_demo::graph::insert_node(&pool, &user, "semantic", "preference",
        "Prefers Rust for backend services", &serde_json::json!({}), 0.9, &prov, &tags).await.unwrap();
    ainm_demo::graph::insert_node(&pool, &user, "semantic", "fact",
        "Name is Santiago", &serde_json::json!({}), 0.95, &prov, &tags).await.unwrap();
    ainm_demo::graph::insert_node(&pool, &user, "semantic", "preference",
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

// ── Briefing: Broad recall — salient unmatched facts still included ───────

#[tokio::test]
async fn briefing_includes_salient_unmatched_facts() {
    let Some(pool) = test_pool().await else { return; };
    let user = unique_addr("bsuf_user");
    let thread = unique_addr("bsuf_thread");
    sqlx::query("INSERT INTO threads (id, origin_domain) VALUES ($1,$2) ON CONFLICT DO NOTHING")
        .bind(&thread).bind("test.local").execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO messages (thread_id, sequence, from_addr, content) VALUES ($1,1,$2,$3)")
        .bind(&thread).bind(&user).bind(serde_json::json!({"text":"help me plan my week"}))
        .execute(&pool).await.unwrap();
    let prov = serde_json::json!({}); let tags = serde_json::json!([]);
    // high-confidence fact with NO lexical overlap with "plan/week"
    ainm_demo::graph::insert_node(&pool,&user,"semantic","fact",
        "Next milestone is connector auth hardening Phase 16",&serde_json::json!({}),0.9,&prov,&tags).await.unwrap();
    let b = ainm_demo::briefing::generate_briefing(&pool,&user,"agent@x",&thread,
        ainm_demo::briefing::ConsentLevel::Standard).await.unwrap();
    let blob = serde_json::to_string(&b).unwrap();
    assert!(blob.contains("Phase 16"), "salient unmatched fact must still be injected: {blob}");
}

// ── Dogfood: Emit briefings for comparison ────────────────────────────────

#[tokio::test]
#[ignore]
async fn dogfood_emit_briefings() {
    let pool = test_pool().await.expect("Postgres must be up for dogfood");
    let user = "dogfood-user@local";
    let thread = "dogfood-thread-week";
    let task = "Help me plan what to work on this week.";

    // clean prior dogfood data (idempotent)
    sqlx::query("DELETE FROM memory_nodes WHERE user_addr = $1").bind(user).execute(&pool).await.unwrap();
    sqlx::query("DELETE FROM messages WHERE thread_id = $1").bind(thread).execute(&pool).await.unwrap();
    sqlx::query("DELETE FROM threads WHERE id = $1").bind(thread).execute(&pool).await.unwrap();

    sqlx::query("INSERT INTO threads (id, origin_domain) VALUES ($1,$2) ON CONFLICT DO NOTHING")
        .bind(thread).bind("local").execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO messages (thread_id, sequence, from_addr, content) VALUES ($1,1,$2,$3)")
        .bind(thread).bind(user).bind(serde_json::json!({"text": task})).execute(&pool).await.unwrap();

    // ~20 realistic facts: (category, label, confidence)
    let facts: Vec<(&str, &str, f32)> = vec![
        ("fact", "Name is Santiago Gericke", 0.98),
        ("fact", "Founder and engineer at TechTorch", 0.95),
        ("fact", "Works primarily in Rust, Swift, and TypeScript", 0.90),
        ("preference", "Prefers Rust for backend services", 0.92),
        ("preference", "Prefers native SwiftUI over React Native for iOS", 0.90),
        ("preference", "Values production-grade implementations over demos", 0.93),
        ("preference", "Prefers terse, direct, critical feedback", 0.88),
        ("preference", "Defaults to test-driven development", 0.85),
        ("fact", "Building AINM, an AI-native messaging app for chatting with many agents", 0.95),
        ("fact", "Current milestone is v3.3 Universal Agent Client", 0.90),
        ("fact", "Building a compound-memory capture layer that compiles conversations into a user profile", 0.92),
        ("fact", "Shipped the v3.2 iOS reference client", 0.90),
        ("decision", "Chose a server-first hybrid architecture for memory compute", 0.85),
        ("decision", "Retired the GSD workflow in favor of superpowers skills", 0.85),
        ("decision", "Treats portable, user-owned memory as the core moat", 0.90),
        ("decision", "Decided hosted runtime is the consent-enforcement boundary", 0.82),
        ("pattern", "Works in focused sprints and pivots based on research", 0.80),
        ("pattern", "Often acts as a critic to pressure-test scope before building", 0.82),
        ("fact", "Wants to validate whether compound memory makes new agents better before investing further", 0.88),
        ("fact", "Next up: connector auth hardening (Phase 16)", 0.80),
    ];
    let prov = serde_json::json!({"source": "dogfood-seed"});
    let tags = serde_json::json!([]);
    for (cat, label, conf) in &facts {
        ainm_demo::graph::insert_node(&pool, user, "semantic", cat, label, &serde_json::json!({}), *conf, &prov, &tags).await.unwrap();
    }

    // B: old dump — no query, no category filter, no ranking
    let dump_nodes = ainm_demo::graph::query_nodes(&pool, user, None, None, None, "current", 0.0, 200).await.unwrap();
    let dump = dump_nodes.iter().map(|n| format!("- {}", n.label)).collect::<Vec<_>>().join("\n");

    // C: new briefing
    let new_briefing = ainm_demo::briefing::generate_briefing(
        &pool, user, "agent@x", thread, ainm_demo::briefing::ConsentLevel::Standard,
    ).await.unwrap();

    let out = serde_json::json!({
        "task": task,
        "arm_a_none": "",
        "arm_b_dump": dump,
        "arm_c_new_briefing": new_briefing,
        "seeded_fact_count": facts.len(),
    });
    let path = "/Users/santiagogerickeparga/Proyectos/ai-native-messaging/experiments/compound-memory-spike/out/dogfood_briefings.json";
    std::fs::create_dir_all("/Users/santiagogerickeparga/Proyectos/ai-native-messaging/experiments/compound-memory-spike/out").unwrap();
    std::fs::write(path, serde_json::to_string_pretty(&out).unwrap()).unwrap();
    println!("=== DOGFOOD BRIEFINGS ===\n{}", serde_json::to_string_pretty(&out).unwrap());
}

// ── Dogfood: SCALE test — briefing surfaces salient facts, drops noise ─────

#[tokio::test]
#[ignore]
async fn dogfood_scale_test() {
    let pool = test_pool().await.expect("Postgres up");
    let user = "dogfood-scale@local";
    let thread = "dogfood-scale-thread";
    let task = "Help me plan what to work on this week.";

    // Clean prior data (idempotent)
    sqlx::query("DELETE FROM memory_nodes WHERE user_addr = $1").bind(user).execute(&pool).await.unwrap();
    sqlx::query("DELETE FROM messages WHERE thread_id = $1").bind(thread).execute(&pool).await.unwrap();
    sqlx::query("DELETE FROM threads WHERE id = $1").bind(thread).execute(&pool).await.unwrap();

    // Seed thread + task message
    sqlx::query("INSERT INTO threads (id, origin_domain) VALUES ($1,$2) ON CONFLICT DO NOTHING")
        .bind(thread).bind("local").execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO messages (thread_id, sequence, from_addr, content) VALUES ($1,1,$2,$3)")
        .bind(thread).bind(user).bind(serde_json::json!({"text": task})).execute(&pool).await.unwrap();

    // 10 IMPORTANT facts — valid_from = NOW() by default (recent) + high confidence
    let prov = serde_json::json!({"source": "dogfood-scale-seed"});
    let tags = serde_json::json!([]);
    let important: Vec<(&str, &str, f32)> = vec![
        ("fact", "[IMP1] Current milestone is v3.3 Universal Agent Client", 0.95),
        ("fact", "[IMP2] Next up: connector auth hardening Phase 16", 0.9),
        ("fact", "[IMP3] Building the compound-memory capture layer this sprint", 0.95),
        ("fact", "[IMP4] Wants to validate compound memory before investing further", 0.9),
        ("decision", "[IMP5] Decided to run a scale test of memory briefing", 0.88),
        ("preference", "[IMP6] Prefers to plan the week around highest-leverage AINM work", 0.9),
        ("fact", "[IMP7] iOS reference client v3.2 shipped, v3.3 in progress", 0.9),
        ("pattern", "[IMP8] Works in focused sprints", 0.85),
        ("fact", "[IMP9] Needs to decide what to build after the memory layer", 0.9),
        ("fact", "[IMP10] Name is Santiago Gericke, founder at TechTorch", 0.98),
    ];
    for (cat, label, conf) in &important {
        ainm_demo::graph::insert_node(&pool, user, "semantic", cat, label, &serde_json::json!({}), *conf, &prov, &tags).await.unwrap();
    }

    // 290 NOISE facts — OLD valid_from (400-700 days) + varied confidence, via raw SQL
    for i in 0..290 {
        let days_old: i64 = 400 + (i as i64 % 300); // 400-699 days old
        let conf: f32 = 0.4 + ((i % 6) as f32) * 0.1; // 0.4..0.9 varied (some high)
        sqlx::query(
            "INSERT INTO memory_nodes (user_addr, node_type, category, label, confidence, valid_from) \
             VALUES ($1,'semantic','fact',$2,$3, NOW() - ($4 || ' days')::interval)"
        )
        .bind(user)
        .bind(format!("Random trivia fact number {i}: an old unrelated note about topic {}", i % 50))
        .bind(conf)
        .bind(days_old.to_string())
        .execute(&pool).await.unwrap();
    }

    let store_total = important.len() + 290;

    // Arm B (dump): no query, no filter, no ranking — the firehose.
    let dump = ainm_demo::graph::query_nodes(&pool, user, None, None, None, "current", 0.0, 200).await.unwrap();
    let arm_b_count = dump.len();
    let arm_b_chars: usize = dump.iter().map(|n| n.label.len()).sum();

    // Arm C (briefing): recency x confidence ranking, query boost, budget pack.
    let c = ainm_demo::briefing::generate_briefing(
        &pool, user, "agent@x", thread, ainm_demo::briefing::ConsentLevel::Standard,
    ).await.unwrap();
    let c_blob = serde_json::to_string(&c).unwrap();

    // C injected fact count = count of facts across all sections arrays.
    let sections = c.get("sections").unwrap().as_object().unwrap();
    let arm_c_fact_count: usize = sections.values()
        .map(|v| v.as_array().map(|a| a.len()).unwrap_or(0))
        .sum();
    let arm_c_char_budget_used = c.get("char_budget_used").unwrap().as_u64().unwrap();
    let arm_c_peer_card = c.get("peer_card").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let mut arm_c_sections_keys: Vec<String> = sections.keys().cloned().collect();
    arm_c_sections_keys.sort();

    // Diagnostic: per-section counts, and which noise ("fact" category) leaked in.
    let mut section_counts: serde_json::Map<String, serde_json::Value> = serde_json::Map::new();
    for (k, v) in sections.iter() {
        section_counts.insert(k.clone(), serde_json::json!(v.as_array().map(|a| a.len()).unwrap_or(0)));
    }
    // Count noise that leaked into C and which trivia indices (recency signal check).
    let mut leaked_noise_indices: Vec<i64> = Vec::new();
    for cap in c_blob.match_indices("Random trivia fact number ") {
        let tail = &c_blob[cap.0 + "Random trivia fact number ".len()..];
        let num: String = tail.chars().take_while(|c| c.is_ascii_digit()).collect();
        if let Ok(n) = num.parse::<i64>() { leaked_noise_indices.push(n); }
    }
    leaked_noise_indices.sort();
    let arm_c_noise_leaked = leaked_noise_indices.len();

    let out = serde_json::json!({
        "task": task,
        "store_total": store_total,
        "arm_b_count": arm_b_count,
        "arm_b_chars": arm_b_chars,
        "arm_c_char_budget_used": arm_c_char_budget_used,
        "arm_c_fact_count": arm_c_fact_count,
        "arm_c_peer_card": arm_c_peer_card,
        "arm_c_sections_keys": arm_c_sections_keys,
        "arm_c_section_counts": section_counts,
        "arm_c_noise_leaked": arm_c_noise_leaked,
        "arm_c_leaked_noise_indices": leaked_noise_indices,
    });
    let dir = "/Users/santiagogerickeparga/Proyectos/ai-native-messaging/experiments/compound-memory-spike/out";
    std::fs::create_dir_all(dir).unwrap();
    let path = format!("{dir}/dogfood_scale.json");
    std::fs::write(&path, serde_json::to_string_pretty(&out).unwrap()).unwrap();
    println!("=== DOGFOOD SCALE ===\n{}", serde_json::to_string_pretty(&out).unwrap());

    // ── Assertions (the proof) ──
    // 1) All 10 important facts surface in C despite 300-fact store + budget
    for i in 1..=10 {
        assert!(c_blob.contains(&format!("[IMP{i}]")), "C must include important fact IMP{i}");
    }
    // 2) C is focused: far fewer facts than the dump, and under budget
    assert!(arm_c_fact_count < 25, "C should inject a focused set, got {arm_c_fact_count}");
    assert!(arm_b_count >= 200, "dump returns the firehose, got {arm_b_count}");
    assert!(arm_c_char_budget_used <= 4000, "C must be under budget, got {arm_c_char_budget_used}");
    // 3) C drops stale noise
    assert!(!c_blob.contains("Random trivia fact number 250"), "stale noise must be excluded from C");
}

// ── Briefing: Consent scoping enforcement ─────────────────────────────────

#[tokio::test]
async fn briefing_minimal_scope_shows_only_identity_categories() {
    let Some(pool) = test_pool().await else { return };
    let user = unique_addr("pcs_min");
    let agent = unique_addr("pcs_min_agent");
    let prov = serde_json::json!({});
    let tags = serde_json::json!([]);
    graph::insert_node(&pool, &user, "semantic", "fact", "Name is Dana", &serde_json::json!({}), 0.95, &prov, &tags).await.unwrap();
    graph::insert_node(&pool, &user, "semantic", "skill", "Expert in Rust", &serde_json::json!({}), 0.9, &prov, &tags).await.unwrap();
    graph::insert_node(&pool, &user, "procedural", "pattern", "Works in sprints", &serde_json::json!({}), 0.85, &prov, &tags).await.unwrap();

    let b = briefing::generate_briefing(&pool, &user, &agent, "pcs_t_min", ConsentLevel::Minimal).await.unwrap();
    let sections = b.get("sections").unwrap().as_object().unwrap();
    assert!(sections.contains_key("fact"), "identity 'fact' must be present at minimal: {sections:?}");
    assert!(!sections.contains_key("skill"), "expertise 'skill' must be excluded at minimal");
    assert!(!sections.contains_key("pattern"), "'pattern' must be excluded at minimal");
}

#[tokio::test]
async fn briefing_standard_scope_excludes_history_and_patterns() {
    let Some(pool) = test_pool().await else { return };
    let user = unique_addr("pcs_std");
    let agent = unique_addr("pcs_std_agent");
    let prov = serde_json::json!({});
    let tags = serde_json::json!([]);
    graph::insert_node(&pool, &user, "semantic", "fact", "Founder", &serde_json::json!({}), 0.95, &prov, &tags).await.unwrap();
    graph::insert_node(&pool, &user, "semantic", "preference", "Likes Rust", &serde_json::json!({}), 0.9, &prov, &tags).await.unwrap();
    graph::insert_node(&pool, &user, "episodic", "decision", "Chose Axum", &serde_json::json!({}), 0.88, &prov, &tags).await.unwrap();
    graph::insert_node(&pool, &user, "episodic", "outcome", "Shipped v1", &serde_json::json!({}), 0.9, &prov, &tags).await.unwrap();
    graph::insert_node(&pool, &user, "procedural", "pattern", "Sprints", &serde_json::json!({}), 0.85, &prov, &tags).await.unwrap();

    let b = briefing::generate_briefing(&pool, &user, &agent, "pcs_t_std", ConsentLevel::Standard).await.unwrap();
    let sections = b.get("sections").unwrap().as_object().unwrap();
    assert!(sections.contains_key("fact"), "standard includes fact");
    assert!(sections.contains_key("preference"), "standard includes preference");
    assert!(sections.contains_key("decision"), "standard includes decision (goals)");
    assert!(!sections.contains_key("outcome"), "outcome is history-only, excluded at standard");
    assert!(!sections.contains_key("pattern"), "pattern is patterns-only, excluded at standard");
}

#[tokio::test]
async fn briefing_full_scope_shows_all_categories() {
    let Some(pool) = test_pool().await else { return };
    let user = unique_addr("pcs_full");
    let agent = unique_addr("pcs_full_agent");
    let prov = serde_json::json!({});
    let tags = serde_json::json!([]);
    graph::insert_node(&pool, &user, "semantic", "fact", "Founder", &serde_json::json!({}), 0.95, &prov, &tags).await.unwrap();
    graph::insert_node(&pool, &user, "episodic", "outcome", "Shipped v1", &serde_json::json!({}), 0.9, &prov, &tags).await.unwrap();
    graph::insert_node(&pool, &user, "procedural", "pattern", "Sprints", &serde_json::json!({}), 0.85, &prov, &tags).await.unwrap();

    let b = briefing::generate_briefing(&pool, &user, &agent, "pcs_t_full", ConsentLevel::Full).await.unwrap();
    let sections = b.get("sections").unwrap().as_object().unwrap();
    assert!(sections.contains_key("fact") && sections.contains_key("outcome") && sections.contains_key("pattern"),
        "full consent surfaces every category: {sections:?}");
}

#[tokio::test]
async fn briefing_unmapped_scope_returns_empty_without_leak() {
    let Some(pool) = test_pool().await else { return };
    let user = unique_addr("pcs_empty");
    let agent = unique_addr("pcs_empty_agent");
    let prov = serde_json::json!({});
    let tags = serde_json::json!([]);
    graph::insert_node(&pool, &user, "semantic", "fact", "Secret name", &serde_json::json!({}), 0.95, &prov, &tags).await.unwrap();

    // A non-empty grant whose scope maps to NO categories (resolve_scopes returns it verbatim
    // because it's non-empty; scope_to_categories("bogus") = vec![] → allowed is empty).
    briefing::grant_scopes(&pool, &user, &agent, &["bogus".to_string()], ConsentLevel::Minimal).await.unwrap();

    let b = briefing::generate_briefing(&pool, &user, &agent, "pcs_t_empty", ConsentLevel::Standard).await.unwrap();
    let sections = b.get("sections").unwrap().as_object().unwrap();
    assert!(sections.is_empty(), "no category granted → empty sections (no leak): {sections:?}");
    assert_eq!(b.get("peer_card").unwrap().as_str().unwrap(), "", "no peer card when nothing granted");
    let blob = serde_json::to_string(&b).unwrap();
    assert!(!blob.contains("Secret name"), "no out-of-scope fact may leak into the briefing");
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

#[tokio::test]
async fn briefing_peer_card_gated_by_identity_scope() {
    let Some(pool) = test_pool().await else { return };
    let user = unique_addr("pcs_pc");
    let agent = unique_addr("pcs_pc_agent");
    let prov = serde_json::json!({});
    let tags = serde_json::json!([]);
    graph::insert_node(&pool, &user, "semantic", "fact", "Name is Dana", &serde_json::json!({}), 0.95, &prov, &tags).await.unwrap();
    graph::insert_node(&pool, &user, "procedural", "pattern", "Works in sprints", &serde_json::json!({}), 0.85, &prov, &tags).await.unwrap();

    // Per-agent grant of ONLY the patterns scope — no identity.
    briefing::grant_scopes(&pool, &user, &agent, &["patterns".to_string()], ConsentLevel::Full).await.unwrap();

    let b = briefing::generate_briefing(&pool, &user, &agent, "pcs_t_pc", ConsentLevel::Standard).await.unwrap();
    let sections = b.get("sections").unwrap().as_object().unwrap();
    assert!(sections.contains_key("pattern"), "granted patterns scope sees pattern facts");
    assert!(!sections.contains_key("fact"), "no identity scope → identity 'fact' excluded");
    assert_eq!(b.get("peer_card").unwrap().as_str().unwrap(), "",
        "peer card must be empty when the identity scope is not granted");
}
