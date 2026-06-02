//! Integration tests for sub-project D (MCP sandboxing).
//!
//! These exercise the public `ainm_server` library surface: the fail-closed skip of a
//! bubblewrap server when `bwrap` is unavailable, and that `UrlGuard` is enforced.

use ainm_server::config::{AppConfig, McpServerConfig, SandboxMode};
use ainm_server::mcp::build_providers_from_config;

fn cfg_with_servers(servers: Vec<McpServerConfig>) -> AppConfig {
    AppConfig {
        database: Default::default(),
        mcp_servers: servers,
        quota: Default::default(),
    }
}

#[tokio::test]
async fn bubblewrap_without_bwrap_is_skipped_not_run_unsandboxed() {
    if ainm_server::sandbox::bwrap_available() && cfg!(target_os = "linux") {
        eprintln!("skipping: host has bwrap; this test asserts the fail-closed path");
        return;
    }
    let config = cfg_with_servers(vec![McpServerConfig {
        name: "untrusted".into(),
        transport: "stdio".into(),
        command: Some("echo".into()),
        args: vec!["should-not-run".into()],
        required_capability: "patterns".into(),
        sandbox: SandboxMode::Bubblewrap,
        ..Default::default()
    }]);
    let providers = build_providers_from_config(&config).await;
    assert!(
        providers.is_empty(),
        "a bubblewrap server without bwrap must be skipped, never run unsandboxed"
    );
}

#[tokio::test]
async fn http_server_with_ssrf_url_is_skipped() {
    let config = cfg_with_servers(vec![McpServerConfig {
        name: "ssrf".into(),
        transport: "http".into(),
        url: Some("http://169.254.169.254/latest/meta-data".into()),
        required_capability: "patterns".into(),
        ..Default::default()
    }]);
    let providers = build_providers_from_config(&config).await;
    assert!(providers.is_empty(), "SSRF URL must be rejected and the server skipped");
}
