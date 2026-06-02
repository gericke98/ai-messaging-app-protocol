use serde::Deserialize;

/// Application configuration loaded from TOML files + environment variables.
///
/// Loading order (later overrides earlier):
/// 1. config/default.toml
/// 2. AINM_* environment variables (nested via __ separator)
///
/// Secrets (database URL, API keys) MUST come from env vars only.
#[derive(Debug, Deserialize, Clone)]
pub struct AppConfig {
    #[serde(default)]
    pub database: DatabaseConfig,

    /// Curated MCP servers the hosted agent runtime may connect to. Empty by default.
    #[serde(default)]
    pub mcp_servers: Vec<McpServerConfig>,

    #[serde(default)]
    pub quota: QuotaConfig,
}

#[derive(Debug, Deserialize, Clone)]
pub struct QuotaConfig {
    #[serde(default = "default_quota_enabled")]
    pub enabled: bool,
    #[serde(default = "default_monthly_token_budget")]
    pub monthly_token_budget: i64,
    #[serde(default = "default_missing_usage_fallback")]
    pub missing_usage_fallback_tokens: u32,
}
impl Default for QuotaConfig {
    fn default() -> Self {
        Self {
            enabled: default_quota_enabled(),
            monthly_token_budget: default_monthly_token_budget(),
            missing_usage_fallback_tokens: default_missing_usage_fallback(),
        }
    }
}
fn default_quota_enabled() -> bool { true }
fn default_monthly_token_budget() -> i64 { 200_000 }
fn default_missing_usage_fallback() -> u32 { 1000 }

/// A single curated (config-defined, trusted) MCP server the hosted agent may use.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct McpServerConfig {
    pub name: String,
    /// "stdio" or "http"
    pub transport: String,
    #[serde(default)]
    pub command: Option<String>,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub url: Option<String>,
    /// The consent capability/scope a requester must hold to be offered this server's tools.
    pub required_capability: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct DatabaseConfig {
    /// PostgreSQL connection URL. Loaded from AINM_DATABASE__URL env var.
    pub url: Option<String>,

    #[serde(default = "default_max_connections")]
    pub max_connections: u32,

    #[serde(default = "default_min_connections")]
    pub min_connections: u32,

    #[serde(default = "default_acquire_timeout")]
    pub acquire_timeout_secs: u64,

    #[serde(default = "default_idle_timeout")]
    pub idle_timeout_secs: u64,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            url: None,
            max_connections: default_max_connections(),
            min_connections: default_min_connections(),
            acquire_timeout_secs: default_acquire_timeout(),
            idle_timeout_secs: default_idle_timeout(),
        }
    }
}

fn default_max_connections() -> u32 { 20 }
fn default_min_connections() -> u32 { 2 }
fn default_acquire_timeout() -> u64 { 5 }
fn default_idle_timeout() -> u64 { 600 }

/// Load configuration from TOML files and environment variables.
pub fn load() -> Result<AppConfig, figment::Error> {
    use figment::{Figment, providers::{Format, Toml, Env}};

    Figment::new()
        .merge(Toml::file("config/default.toml"))
        .merge(Env::prefixed("AINM_").split("__"))
        .extract()
}
