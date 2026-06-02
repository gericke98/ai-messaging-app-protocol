//! Tool abstraction for the hosted agent runtime.
//!
//! A `ToolProvider` advertises tools (filtered to the requester's consent scopes) and
//! executes calls. The MCP client (mcp.rs) lives behind this same trait so sub-project D
//! can replace it with a sandboxed executor without touching the runtime loop.

use crate::llm::ToolSchema;

/// The result of a tool call, fed back to the model as a `tool` message.
#[derive(Debug, Clone)]
pub struct ToolResult {
    pub content: String,
    pub is_error: bool,
}

impl ToolResult {
    pub fn ok(content: impl Into<String>) -> Self { Self { content: content.into(), is_error: false } }
    pub fn error(content: impl Into<String>) -> Self { Self { content: content.into(), is_error: true } }
}

/// A source of tools. Implementations: memory tools (in-process), MCP bridge.
#[async_trait::async_trait]
pub trait ToolProvider: Send + Sync {
    /// Tools this provider offers GIVEN the requester's granted scopes (already consent-filtered).
    async fn list_tools(&self, scopes: &[String]) -> Vec<ToolSchema>;
    /// Execute a tool. Implementations MUST re-check consent and refuse disallowed calls
    /// with `ToolResult::error` rather than executing.
    async fn call(&self, name: &str, args: &serde_json::Value, scopes: &[String]) -> ToolResult;
}

/// Aggregates multiple providers and dispatches by tool name.
pub struct ToolRegistry {
    providers: Vec<Box<dyn ToolProvider>>,
}

impl ToolRegistry {
    pub fn new(providers: Vec<Box<dyn ToolProvider>>) -> Self { Self { providers } }

    /// The union of all providers' consent-filtered tools.
    pub async fn list_tools(&self, scopes: &[String]) -> Vec<ToolSchema> {
        let mut out = Vec::new();
        for p in &self.providers {
            out.extend(p.list_tools(scopes).await);
        }
        out
    }

    /// Dispatch a call to the provider that offers `name`. If no provider offers it
    /// for these scopes, refuse with a tool-error (never silently execute).
    pub async fn call(&self, name: &str, args: &serde_json::Value, scopes: &[String]) -> ToolResult {
        for p in &self.providers {
            let offered = p.list_tools(scopes).await;
            if offered.iter().any(|t| t.name == name) {
                return p.call(name, args, scopes).await;
            }
        }
        ToolResult::error(format!("tool '{name}' is not available (not offered for current consent scopes)"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct EchoProvider;
    #[async_trait::async_trait]
    impl ToolProvider for EchoProvider {
        async fn list_tools(&self, _scopes: &[String]) -> Vec<crate::llm::ToolSchema> {
            vec![crate::llm::ToolSchema {
                name: "echo".into(),
                description: "echoes input".into(),
                parameters: serde_json::json!({"type":"object","properties":{"msg":{"type":"string"}}}),
            }]
        }
        async fn call(&self, name: &str, args: &serde_json::Value, _scopes: &[String]) -> ToolResult {
            if name == "echo" {
                ToolResult::ok(args["msg"].as_str().unwrap_or("").to_string())
            } else {
                ToolResult::error(format!("unknown tool: {name}"))
            }
        }
    }

    #[tokio::test]
    async fn registry_lists_and_dispatches() {
        let reg = ToolRegistry::new(vec![Box::new(EchoProvider)]);
        let scopes = vec!["identity".to_string()];
        let tools = reg.list_tools(&scopes).await;
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name, "echo");

        let res = reg.call("echo", &serde_json::json!({"msg": "hi"}), &scopes).await;
        assert!(!res.is_error);
        assert_eq!(res.content, "hi");

        let missing = reg.call("nope", &serde_json::json!({}), &scopes).await;
        assert!(missing.is_error);
    }
}
