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

    #[serde(default)]
    pub a2a: A2aConfig,
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

/// Runtime A2A (agent-to-agent) policy knobs.
#[derive(Debug, Deserialize, Clone)]
pub struct A2aConfig {
    /// Max consecutive agent→agent hops in a thread with no human between.
    /// Loaded from `AINM_A2A__HOP_CAP`. Default 6.
    #[serde(default = "default_a2a_hop_cap")]
    pub hop_cap: u32,
}
impl Default for A2aConfig {
    fn default() -> Self {
        Self { hop_cap: default_a2a_hop_cap() }
    }
}
fn default_a2a_hop_cap() -> u32 { 6 }

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

    // --- sub-project D: sandboxing ---
    /// Isolation backend. Defaults to `None` (backward-compat); hardening (env scrub,
    /// rlimits, timeout, output cap) applies regardless. Non-first-party servers should
    /// set `sandbox = "bubblewrap"`.
    #[serde(default)]
    pub sandbox: SandboxMode,
    /// Allow the sandboxed process network access (bubblewrap: adds `--share-net`).
    #[serde(default)]
    pub allow_network: bool,
    /// Read-only bind mounts granted to the sandbox.
    #[serde(default)]
    pub allow_read: Vec<String>,
    /// Read-write bind mounts granted to the sandbox.
    #[serde(default)]
    pub allow_write: Vec<String>,
    /// Explicit child environment (TOML inline table: `env = { KEY = "val" }`).
    /// The child NEVER inherits the server's environment; only these + a minimal allowlist.
    #[serde(default)]
    pub env: std::collections::BTreeMap<String, String>,
    /// HTTP transport only: SSRF allowlist of hostnames exempt from `UrlGuard` private-IP rejection.
    #[serde(default)]
    pub allowed_hosts: Vec<String>,
    /// Resource limits applied via setrlimit + per-call caps.
    #[serde(default)]
    pub limits: SandboxLimits,
}

/// Isolation backend for an MCP server.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum SandboxMode {
    /// Hardening-only (env scrub + rlimits + timeout + output cap), no namespace isolation.
    #[default]
    None,
    /// Linux bubblewrap: namespaces + seccomp + deny-all fs/net + explicit grants.
    Bubblewrap,
}

/// Resource limits for a sandboxed MCP server.
#[derive(Debug, Clone, Deserialize)]
pub struct SandboxLimits {
    #[serde(default = "default_max_memory_mb")]
    pub max_memory_mb: u64, // RLIMIT_AS
    #[serde(default = "default_max_cpu_secs")]
    pub max_cpu_secs: u64, // RLIMIT_CPU
    #[serde(default = "default_max_procs")]
    pub max_procs: u64, // RLIMIT_NPROC
    #[serde(default = "default_max_nofile")]
    pub max_nofile: u64, // RLIMIT_NOFILE
    #[serde(default = "default_max_fsize_mb")]
    pub max_fsize_mb: u64, // RLIMIT_FSIZE
    #[serde(default = "default_max_output_bytes")]
    pub max_output_bytes: usize, // per-call output cap
    #[serde(default = "default_wall_timeout_secs")]
    pub wall_timeout_secs: u64, // per-call wall-clock timeout
}
impl Default for SandboxLimits {
    fn default() -> Self {
        Self {
            max_memory_mb: default_max_memory_mb(),
            max_cpu_secs: default_max_cpu_secs(),
            max_procs: default_max_procs(),
            max_nofile: default_max_nofile(),
            max_fsize_mb: default_max_fsize_mb(),
            max_output_bytes: default_max_output_bytes(),
            wall_timeout_secs: default_wall_timeout_secs(),
        }
    }
}
fn default_max_memory_mb() -> u64 { 512 }
fn default_max_cpu_secs() -> u64 { 30 }
fn default_max_procs() -> u64 { 64 }
fn default_max_nofile() -> u64 { 256 }
fn default_max_fsize_mb() -> u64 { 64 }
fn default_max_output_bytes() -> usize { 1_048_576 }
fn default_wall_timeout_secs() -> u64 { 60 }

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
#[allow(clippy::result_large_err)] // figment::Error is large; load() runs once at startup
pub fn load() -> Result<AppConfig, figment::Error> {
    use figment::{Figment, providers::{Format, Toml, Env}};

    Figment::new()
        .merge(Toml::file("config/default.toml"))
        .merge(Env::prefixed("AINM_").split("__"))
        .extract()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mcp_server_config_d_fields_default() {
        let toml = r#"
            name = "weather"
            transport = "stdio"
            required_capability = "patterns"
        "#;
        let cfg: McpServerConfig = toml::from_str(toml).unwrap();
        assert_eq!(cfg.sandbox, SandboxMode::None);
        assert!(!cfg.allow_network);
        assert!(cfg.allow_read.is_empty());
        assert!(cfg.allow_write.is_empty());
        assert!(cfg.env.is_empty());
        assert!(cfg.allowed_hosts.is_empty());
        assert_eq!(cfg.limits.max_memory_mb, 512);
        assert_eq!(cfg.limits.max_cpu_secs, 30);
        assert_eq!(cfg.limits.max_procs, 64);
        assert_eq!(cfg.limits.max_nofile, 256);
        assert_eq!(cfg.limits.max_fsize_mb, 64);
        assert_eq!(cfg.limits.max_output_bytes, 1_048_576);
        assert_eq!(cfg.limits.wall_timeout_secs, 60);
    }

    #[test]
    fn mcp_server_config_d_fields_parse() {
        let toml = r#"
            name = "weather"
            transport = "stdio"
            command = "uvx"
            args = ["mcp-weather"]
            required_capability = "patterns"
            sandbox = "bubblewrap"
            allow_network = true
            allow_read = ["/opt/data"]
            allowed_hosts = ["api.weather.example"]
            env = { WEATHER_UNITS = "metric" }

            [limits]
            max_memory_mb = 256
            wall_timeout_secs = 15
        "#;
        let cfg: McpServerConfig = toml::from_str(toml).unwrap();
        assert_eq!(cfg.sandbox, SandboxMode::Bubblewrap);
        assert!(cfg.allow_network);
        assert_eq!(cfg.allow_read, vec!["/opt/data".to_string()]);
        assert_eq!(cfg.allowed_hosts, vec!["api.weather.example".to_string()]);
        assert_eq!(cfg.env.get("WEATHER_UNITS").map(String::as_str), Some("metric"));
        assert_eq!(cfg.limits.max_memory_mb, 256);
        assert_eq!(cfg.limits.wall_timeout_secs, 15);
        assert_eq!(cfg.limits.max_cpu_secs, 30);
    }

    #[test]
    fn a2a_hop_cap_defaults_to_six() {
        let cfg = A2aConfig::default();
        assert_eq!(cfg.hop_cap, 6);
    }

    #[test]
    fn a2a_hop_cap_parses_from_toml() {
        let cfg: A2aConfig = toml::from_str("hop_cap = 3").unwrap();
        assert_eq!(cfg.hop_cap, 3);
    }
}
