//! Curated MCP bridge (full impl in B-9). Stub returns no providers so B-8 compiles.

use crate::config::AppConfig;
use crate::tools::ToolProvider;
use std::sync::Arc;

/// Build ToolProviders from curated MCP server config. Stub: none yet (B-9 fills this in).
pub async fn build_providers_from_config(_config: &AppConfig) -> Vec<Arc<dyn ToolProvider>> {
    vec![]
}
