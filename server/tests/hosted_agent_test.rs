//! Integration tests for the hosted agent runtime (sub-project B).

use ainm_server::hosted_agent;

/// Acquire a test pool or skip. Mirrors the helper used by the compound-memory
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

#[tokio::test]
async fn bootstrap_is_idempotent() {
    let Some(pool) = pool_or_skip().await else {
        eprintln!("no DB; skipping");
        return;
    };
    let domain = "ainm.local";

    let a1 = hosted_agent::provision(&pool, domain).await.expect("provision 1");
    let a2 = hosted_agent::provision(&pool, domain).await.expect("provision 2");

    assert_eq!(a1.address, format!("assistant@{domain}"));
    assert_eq!(a1.public_key_bytes, a2.public_key_bytes);

    let acct_count: i64 = sqlx::query_scalar("SELECT count(*) FROM accounts WHERE address = $1")
        .bind(&a1.address)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(acct_count, 1);
    let ha_count: i64 = sqlx::query_scalar("SELECT count(*) FROM hosted_agents WHERE address = $1")
        .bind(&a1.address)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(ha_count, 1);
}

#[tokio::test]
async fn consent_gate_hides_ungranted_tools() {
    let minimal = vec!["identity".to_string()];
    let offered = ainm_server::tools_memory::offered_tool_names(&minimal);
    assert!(offered.contains(&"query_memory".to_string()));
    assert!(!offered.contains(&"store_fact".to_string()));

    let none: Vec<String> = vec![];
    assert!(ainm_server::tools_memory::offered_tool_names(&none).is_empty());
}

#[tokio::test]
async fn full_hosted_turn_with_mock_llm_and_mock_tool() {
    use ainm_server::llm::{AssistantTurn, MockLlmClient, ToolCall, ToolSchema};
    use ainm_server::runtime::run_loop;
    use ainm_server::tools::{ToolProvider, ToolRegistry, ToolResult};

    struct MockMcp;
    #[async_trait::async_trait]
    impl ToolProvider for MockMcp {
        async fn list_tools(&self, scopes: &[String]) -> Vec<ToolSchema> {
            if scopes.iter().any(|s| s == "patterns") {
                vec![ToolSchema {
                    name: "get_time".into(),
                    description: "t".into(),
                    parameters: serde_json::json!({"type":"object"}),
                }]
            } else {
                vec![]
            }
        }
        async fn call(&self, name: &str, _a: &serde_json::Value, scopes: &[String]) -> ToolResult {
            if !scopes.iter().any(|s| s == "patterns") {
                return ToolResult::error("denied");
            }
            if name == "get_time" {
                ToolResult::ok("2026-06-01T12:00:00Z")
            } else {
                ToolResult::error("no")
            }
        }
    }

    let llm = MockLlmClient::with_turns(vec![
        AssistantTurn::ToolCalls(vec![ToolCall {
            id: "c1".into(),
            name: "get_time".into(),
            arguments: serde_json::json!({}),
        }]),
        AssistantTurn::Text("It is noon UTC.".into()),
    ]);
    let reg = ToolRegistry::new(vec![Box::new(MockMcp)]);

    let out = run_loop(&llm, &reg, "sys", &["patterns".to_string()], vec![], "what time is it?", 8).await;
    assert_eq!(out.final_text, "It is noon UTC.");
    assert_eq!(out.tool_rounds, 1);
}

#[tokio::test]
async fn mcp_tool_absent_without_capability() {
    use ainm_server::llm::ToolSchema;
    use ainm_server::tools::{ToolProvider, ToolRegistry};

    struct MockMcp;
    #[async_trait::async_trait]
    impl ToolProvider for MockMcp {
        async fn list_tools(&self, scopes: &[String]) -> Vec<ToolSchema> {
            if scopes.iter().any(|s| s == "patterns") {
                vec![ToolSchema {
                    name: "get_time".into(),
                    description: "t".into(),
                    parameters: serde_json::json!({"type":"object"}),
                }]
            } else {
                vec![]
            }
        }
        async fn call(
            &self,
            _n: &str,
            _a: &serde_json::Value,
            _s: &[String],
        ) -> ainm_server::tools::ToolResult {
            ainm_server::tools::ToolResult::error("denied")
        }
    }
    let reg = ToolRegistry::new(vec![Box::new(MockMcp)]);
    assert!(reg.list_tools(&["identity".to_string()]).await.is_empty());
    let refused = reg
        .call("get_time", &serde_json::json!({}), &["identity".to_string()])
        .await;
    assert!(refused.is_error);
}

#[tokio::test]
async fn loop_stops_at_cap_integration() {
    use ainm_server::llm::{AssistantTurn, MockLlmClient, ToolCall, ToolSchema};
    use ainm_server::runtime::run_loop;
    use ainm_server::tools::{ToolProvider, ToolRegistry, ToolResult};

    struct AlwaysTool;
    #[async_trait::async_trait]
    impl ToolProvider for AlwaysTool {
        async fn list_tools(&self, _s: &[String]) -> Vec<ToolSchema> {
            vec![ToolSchema {
                name: "t".into(),
                description: "t".into(),
                parameters: serde_json::json!({"type":"object"}),
            }]
        }
        async fn call(&self, _n: &str, _a: &serde_json::Value, _s: &[String]) -> ToolResult {
            ToolResult::ok("x")
        }
    }
    let turns = (0..20)
        .map(|_| {
            AssistantTurn::ToolCalls(vec![ToolCall {
                id: "c".into(),
                name: "t".into(),
                arguments: serde_json::json!({}),
            }])
        })
        .collect();
    let llm = MockLlmClient::with_turns(turns);
    let reg = ToolRegistry::new(vec![Box::new(AlwaysTool)]);
    let out = run_loop(&llm, &reg, "sys", &["identity".to_string()], vec![], "go", 3).await;
    assert_eq!(out.tool_rounds, 3);
    assert!(out.stopped_early);
}
