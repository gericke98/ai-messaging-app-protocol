//! Curated MCP bridge: connects to trusted MCP servers (config-defined) and exposes
//! their tools through the `ToolProvider` trait. This is the sub-project-D isolation seam —
//! D can replace this impl with a sandboxed executor without touching the runtime loop.
//!
//! ALL `rmcp` types are confined to `McpClientHandle` below. No other module imports `rmcp`.

use crate::config::{AppConfig, McpServerConfig};
use crate::llm::ToolSchema;
use crate::tools::{ToolProvider, ToolResult};
use std::sync::Arc;

/// Pure consent check: is this curated server offered for the requester's scopes?
pub fn server_is_permitted(cfg: &McpServerConfig, scopes: &[String]) -> bool {
    scopes.iter().any(|s| s == &cfg.required_capability)
}

/// Build one ToolProvider per configured curated MCP server. Connection failures are
/// logged and that server is skipped (the runtime still works without it).
pub async fn build_providers_from_config(config: &AppConfig) -> Vec<Arc<dyn ToolProvider>> {
    let mut providers: Vec<Arc<dyn ToolProvider>> = Vec::new();
    for cfg in &config.mcp_servers {
        match McpToolProvider::connect(cfg.clone()).await {
            Ok(p) => {
                tracing::info!(
                    "[mcp] connected curated server '{}' ({} tools)",
                    cfg.name,
                    p.tools.len()
                );
                providers.push(Arc::new(p));
            }
            Err(e) => {
                tracing::warn!("[mcp] skipping server '{}' (not started): {e}", cfg.name)
            }
        }
    }
    providers
}

/// A connected curated MCP server exposed as a ToolProvider.
pub struct McpToolProvider {
    cfg: McpServerConfig,
    /// Tools discovered at connect time (so `list_tools` is cheap — no I/O).
    tools: Vec<ToolSchema>,
    client: McpClientHandle,
}

impl McpToolProvider {
    pub async fn connect(cfg: McpServerConfig) -> anyhow::Result<Self> {
        let client = McpClientHandle::connect(&cfg).await?;
        let tools = client.list_tools().await?;
        Ok(Self { cfg, tools, client })
    }
}

#[async_trait::async_trait]
impl ToolProvider for McpToolProvider {
    fn name(&self) -> &'static str {
        "mcp"
    }

    async fn list_tools(&self, scopes: &[String]) -> Vec<ToolSchema> {
        if !server_is_permitted(&self.cfg, scopes) {
            return vec![];
        }
        self.tools.clone()
    }

    async fn call(&self, name: &str, args: &serde_json::Value, scopes: &[String]) -> ToolResult {
        if !server_is_permitted(&self.cfg, scopes) {
            return ToolResult::error(format!(
                "MCP server '{}' not permitted by consent scopes",
                self.cfg.name
            ));
        }
        if !self.tools.iter().any(|t| t.name == name) {
            return ToolResult::error(format!(
                "tool '{name}' not offered by '{}'",
                self.cfg.name
            ));
        }
        match self.client.call_tool(name, args, &self.cfg.limits).await {
            Ok(text) => ToolResult::ok(text),
            Err(e) => ToolResult::error(format!("MCP call failed: {e}")),
        }
    }
}

/// Thin wrapper around the rmcp client — the ONLY place that imports `rmcp`.
struct McpClientHandle {
    inner: rmcp::service::RunningService<rmcp::RoleClient, ()>,
}

impl McpClientHandle {
    async fn connect(cfg: &McpServerConfig) -> anyhow::Result<Self> {
        use rmcp::ServiceExt;
        let inner = match cfg.transport.as_str() {
            "stdio" => {
                use rmcp::transport::TokioChildProcess;
                // sub-project D: build a hardened (and possibly bubblewrap-wrapped) command.
                let launcher = crate::sandbox::launcher_for(cfg)?;
                let command = launcher.build_command(cfg)?;
                tracing::debug!(
                    "[mcp] launching '{}' via {} launcher",
                    cfg.name,
                    launcher.name()
                );
                let transport = TokioChildProcess::new(command)?;
                ().serve(transport).await?
            }
            "http" => {
                use rmcp::transport::StreamableHttpClientTransport;
                let url = cfg.url.clone().ok_or_else(|| {
                    anyhow::anyhow!("http MCP server '{}' missing url", cfg.name)
                })?;
                // sub-project D: anti-SSRF connect-time check.
                crate::sandbox::UrlGuard::check(&url, &cfg.allowed_hosts)?;
                let transport = StreamableHttpClientTransport::from_uri(url);
                ().serve(transport).await?
            }
            other => anyhow::bail!("unknown MCP transport '{other}' for '{}'", cfg.name),
        };
        Ok(Self { inner })
    }

    async fn list_tools(&self) -> anyhow::Result<Vec<ToolSchema>> {
        let tools = self.inner.list_all_tools().await?;
        Ok(tools
            .into_iter()
            .map(|t| ToolSchema {
                name: t.name.to_string(),
                description: t.description.map(|d| d.to_string()).unwrap_or_default(),
                // `input_schema` is an `Arc<serde_json::Map<String, Value>>`.
                parameters: serde_json::Value::Object((*t.input_schema).clone()),
            })
            .collect())
    }

    async fn call_tool(
        &self,
        name: &str,
        args: &serde_json::Value,
        limits: &crate::config::SandboxLimits,
    ) -> anyhow::Result<String> {
        use rmcp::model::CallToolRequestParams;
        // `CallToolRequestParams` is `#[non_exhaustive]`; build via the constructor then set args.
        let mut params = CallToolRequestParams::new(name.to_string());
        params.arguments = args.as_object().cloned();

        let fut = self.inner.call_tool(params);
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(limits.wall_timeout_secs),
            fut,
        )
        .await
        .map_err(|_| anyhow::anyhow!("MCP tool '{name}' timed out after {}s", limits.wall_timeout_secs))??;

        let text = extract_content_text(&result.content);
        if result.is_error == Some(true) {
            return Err(anyhow::anyhow!("MCP tool reported an error: {text}"));
        }
        Ok(cap_output(&text, limits.max_output_bytes))
    }
}

/// Truncate tool output to at most `max_bytes`, appending a marker if truncated.
/// Truncation respects UTF-8 char boundaries (never splits a multibyte char).
fn cap_output(text: &str, max_bytes: usize) -> String {
    if text.len() <= max_bytes {
        return text.to_string();
    }
    let mut end = max_bytes;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…[truncated]", &text[..end])
}

/// Extract human-/model-readable text from MCP content blocks.
///
/// `Content` is `Annotated<RawContent>`; text blocks expose their string via
/// `content.raw.as_text()` (`RawContent::as_text -> Option<&RawTextContent>`).
fn extract_content_text(content: &[rmcp::model::Content]) -> String {
    let texts: Vec<&str> = content
        .iter()
        .filter_map(|c| c.raw.as_text().map(|t| t.text.as_str()))
        .collect();
    if texts.is_empty() {
        // Fallback: no text blocks (e.g. image/resource) — give the model the JSON so it isn't blind.
        serde_json::to_string(content).unwrap_or_default()
    } else {
        texts.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mcp_server_offered_only_when_capability_granted() {
        let cfg = crate::config::McpServerConfig {
            name: "weather".into(),
            transport: "http".into(),
            command: None,
            args: vec![],
            url: Some("http://localhost:9000/mcp".into()),
            required_capability: "patterns".into(),
            ..Default::default()
        };
        assert!(server_is_permitted(&cfg, &["patterns".to_string()]));
        assert!(!server_is_permitted(&cfg, &["identity".to_string()]));
        assert!(!server_is_permitted(&cfg, &[]));
    }

    #[test]
    fn output_cap_truncates_with_marker() {
        let capped = super::cap_output("hello world", 5);
        assert!(capped.starts_with("hello"));
        assert!(capped.ends_with("…[truncated]"));
        assert_eq!(super::cap_output("hi", 100), "hi");
    }

    #[tokio::test]
    async fn build_providers_skips_bubblewrap_without_bwrap() {
        let config = crate::config::AppConfig {
            database: Default::default(),
            mcp_servers: vec![crate::config::McpServerConfig {
                name: "needs-sandbox".into(),
                transport: "stdio".into(),
                command: Some("echo".into()),
                args: vec![],
                required_capability: "patterns".into(),
                sandbox: crate::config::SandboxMode::Bubblewrap,
                ..Default::default()
            }],
            quota: Default::default(),
        };
        if !crate::sandbox::bwrap_available() || !cfg!(target_os = "linux") {
            let providers = build_providers_from_config(&config).await;
            assert!(providers.is_empty(), "unsandboxed bubblewrap server must be skipped");
        }
    }
}
