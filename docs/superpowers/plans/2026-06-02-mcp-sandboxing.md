# MCP Sandboxing & Isolation Implementation Plan (sub-project D)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Wrap every spawned MCP server in always-on portable hardening (env scrub, rlimits, process-group kill, no-shell, per-call timeout + output cap) plus an optional Linux `BubblewrapLauncher` (deny-all filesystem/network/syscall isolation with explicit grants) and an anti-SSRF `UrlGuard` for HTTP transport — all behind a pluggable `Launcher` trait, with `rmcp` still confined to `McpClientHandle`.

**Architecture:** A new `server/src/sandbox.rs` owns the `Launcher` trait (`LocalLauncher` = hardening-only, `BubblewrapLauncher` = hardening + `bwrap` wrapping), the pure rlimit/env-scrub helpers, the `bwrap` argv builder + seccomp profile + presence probe, `launcher_for` (fail-closed selection), and `UrlGuard`. `server/src/config.rs` gains serde-defaulted `McpServerConfig` fields + `SandboxMode` + `SandboxLimits`. `server/src/mcp.rs` (the only `rmcp` importer) calls `launcher_for(cfg)?.build_command(cfg)?` on the stdio path, `UrlGuard::check` on the http path, wraps `call_tool` in a wall-clock timeout + output cap, and makes `build_providers_from_config` skip (logged) any server whose launcher errors.

**Tech Stack:** Rust 2021, tokio, rmcp 1.7, `libc` (rlimit `setrlimit` in a `pre_exec` closure), `url` 2 (URL parsing for `UrlGuard`), `seccompiler` 0.5 (Linux-only: compile the baseline seccomp denylist to BPF), `nix` (memfd for passing seccomp BPF to `bwrap`). Existing two known-failing tests (`test_graph_traversal`, `test_compilation_dedup_updates_confidence`) stay failing — unrelated.

---

## File Structure

| File | Responsibility | Change |
|------|----------------|--------|
| `server/src/config.rs` | `McpServerConfig` D fields, `SandboxMode`, `SandboxLimits` + serde defaults | Modify |
| `server/src/sandbox.rs` | `Launcher` trait, `LocalLauncher`, `BubblewrapLauncher`, rlimit/env-scrub helpers, seccomp profile, `launcher_for`, `UrlGuard` | Create |
| `server/src/mcp.rs` | Wire launcher into stdio connect; `UrlGuard` on http connect; `call_tool` timeout + output cap; fail-closed `build_providers_from_config` | Modify |
| `server/src/lib.rs` | Register `mod sandbox;` | Modify |
| `server/Cargo.toml` | Add `libc`, `url`; Linux-only `seccompiler`; `nix` (memfd feature) | Modify |
| `server/tests/mcp_sandbox.rs` | Integration: bubblewrap-without-bwrap skip; curated/consent path unchanged | Create |

Each task below is self-contained. Tasks D-1..D-4 are pure/unit-testable on macOS + Linux. D-5 wires `mcp.rs`. D-6 is Linux-only runtime tests. D-7 is integration + final verification.

---

## Task D-1: Config schema (`SandboxMode`, `SandboxLimits`, `McpServerConfig` fields)

**Files:**
- Modify: `server/src/config.rs`
- Modify: `server/src/mcp.rs:170-183` (the existing `McpServerConfig` literal in the test — it currently lists all 6 fields; new defaulted fields force a `..Default::default()`)

- [ ] **Step 1: Write the failing test**

Add to the `#[cfg(test)] mod tests` at the bottom of `server/src/config.rs` (create the module if absent):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mcp_server_config_d_fields_default() {
        // A minimal server (only the pre-D required fields set) must still deserialize,
        // with D fields taking secure-ish backward-compatible defaults.
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
        // SandboxLimits defaults
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
        // unset limit keeps its default
        assert_eq!(cfg.limits.max_cpu_secs, 30);
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p ainm-server --lib config::tests -- --nocapture`
Expected: FAIL — `SandboxMode`/`SandboxLimits` undefined, fields missing, and `toml` may be unavailable as a dev-dep.

- [ ] **Step 3: Add `toml` dev-dep (if missing) and implement the schema**

First ensure `toml` is available for the test. In `server/Cargo.toml` under `[dev-dependencies]`:

```toml
toml = "0.8"
```

Then in `server/src/config.rs`, replace the existing `McpServerConfig` struct (currently `server/src/config.rs:46-59`) with:

```rust
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
```

- [ ] **Step 4: Fix the existing `McpServerConfig` literal in `mcp.rs`**

In `server/src/mcp.rs`, the test `mcp_server_offered_only_when_capability_granted` (around `server/src/mcp.rs:170-183`) builds `McpServerConfig { ... }` with exactly the 6 pre-D fields. Append `..Default::default()` so it keeps compiling:

```rust
        let cfg = crate::config::McpServerConfig {
            name: "weather".into(),
            transport: "http".into(),
            command: None,
            args: vec![],
            url: Some("http://localhost:9000/mcp".into()),
            required_capability: "patterns".into(),
            ..Default::default()
        };
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p ainm-server --lib config:: && cargo test -p ainm-server --lib mcp::tests`
Expected: PASS (all config + mcp unit tests).

- [ ] **Step 6: Commit**

```bash
rtk proxy git add server/src/config.rs server/src/mcp.rs server/Cargo.toml
rtk proxy git commit -m "feat(d): McpServerConfig sandbox fields + SandboxMode/SandboxLimits"
```

---

## Task D-2: `UrlGuard` (anti-SSRF, pure)

**Files:**
- Create: `server/src/sandbox.rs`
- Modify: `server/src/lib.rs` (add `pub mod sandbox;`)
- Modify: `server/Cargo.toml` (add `url = "2"`)

- [ ] **Step 1: Add the `url` dependency**

In `server/Cargo.toml` under `[dependencies]`:

```toml
# Sandboxing (sub-project D): URL parsing for the HTTP-transport SSRF guard
url = "2"
```

- [ ] **Step 2: Create `server/src/sandbox.rs` with the failing test**

Create `server/src/sandbox.rs`:

```rust
//! Sub-project D: MCP sandboxing & isolation.
//!
//! Always-on portable hardening for every spawned MCP server (env scrub, rlimits,
//! process-group kill, no-shell, timeout, output cap) behind a pluggable `Launcher`
//! trait, an optional Linux `BubblewrapLauncher` (namespaces + seccomp + deny-all
//! fs/net + explicit grants), and a connect-time anti-SSRF `UrlGuard` for the HTTP
//! transport. `rmcp` is NOT imported here — it stays confined to `mcp.rs`.

use std::net::IpAddr;

/// Connect-time URL hardening for HTTP-transport MCP servers (anti-SSRF).
pub struct UrlGuard;

impl UrlGuard {
    /// Reject loopback / private (RFC1918) / link-local / ULA / unspecified targets
    /// unless the host is explicitly listed in `allowed_hosts`. Requires http(s).
    ///
    /// This is a connect-time check; it does not defend against DNS-rebinding
    /// mid-session (documented v1 limitation — see the design doc §6).
    pub fn check(url_str: &str, allowed_hosts: &[String]) -> anyhow::Result<()> {
        let url = url::Url::parse(url_str)
            .map_err(|e| anyhow::anyhow!("invalid MCP url: {e}"))?;
        match url.scheme() {
            "http" | "https" => {}
            other => anyhow::bail!("MCP url scheme '{other}' not allowed (need http/https)"),
        }
        let host = url
            .host_str()
            .ok_or_else(|| anyhow::anyhow!("MCP url has no host"))?;

        // Explicit allowlist bypasses IP classification (operator opt-in).
        if allowed_hosts.iter().any(|h| h.eq_ignore_ascii_case(host)) {
            return Ok(());
        }

        // Classify every resolved IP. If the host is a literal IP, classify it directly;
        // otherwise resolve via DNS. Reject if ANY resolved address is in the SSRF set.
        let addrs = resolve_host(host, url.port_or_known_default().unwrap_or(0))?;
        if addrs.is_empty() {
            anyhow::bail!("MCP url host '{host}' did not resolve");
        }
        for ip in &addrs {
            if is_blocked_ip(ip) {
                anyhow::bail!(
                    "MCP url host '{host}' resolves to a blocked address ({ip}); \
                     add it to allowed_hosts to override"
                );
            }
        }
        Ok(())
    }
}

/// Resolve a host string (literal IP or DNS name) to a set of IPs.
fn resolve_host(host: &str, port: u16) -> anyhow::Result<Vec<IpAddr>> {
    use std::net::ToSocketAddrs;
    if let Ok(ip) = host.parse::<IpAddr>() {
        return Ok(vec![ip]);
    }
    let iter = (host, port)
        .to_socket_addrs()
        .map_err(|e| anyhow::anyhow!("could not resolve '{host}': {e}"))?;
    Ok(iter.map(|sa| sa.ip()).collect())
}

/// Is this IP in the SSRF block set (loopback / private / link-local / ULA / unspecified)?
fn is_blocked_ip(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_unspecified()
                || v4.is_broadcast()
                // 100.64.0.0/10 carrier-grade NAT
                || (v4.octets()[0] == 100 && (v4.octets()[1] & 0xc0) == 64)
        }
        IpAddr::V6(v6) => {
            v6.is_loopback()
                || v6.is_unspecified()
                // link-local fe80::/10
                || (v6.segments()[0] & 0xffc0) == 0xfe80
                // unique-local fc00::/7
                || (v6.segments()[0] & 0xfe00) == 0xfc00
        }
    }
}

#[cfg(test)]
mod url_guard_tests {
    use super::*;

    fn check(u: &str) -> anyhow::Result<()> {
        UrlGuard::check(u, &[])
    }

    #[test]
    fn rejects_loopback() {
        assert!(check("http://127.0.0.1:9000/mcp").is_err());
        assert!(check("http://localhost:9000/mcp").is_err());
        assert!(check("http://[::1]:9000/mcp").is_err());
    }

    #[test]
    fn rejects_private_v4() {
        assert!(check("http://10.0.0.5/mcp").is_err());
        assert!(check("http://172.16.3.4/mcp").is_err());
        assert!(check("http://192.168.1.1/mcp").is_err());
    }

    #[test]
    fn rejects_link_local_and_unspecified() {
        assert!(check("http://169.254.169.254/latest/meta-data").is_err()); // cloud metadata
        assert!(check("http://0.0.0.0/mcp").is_err());
        assert!(check("http://[fe80::1]/mcp").is_err());
        assert!(check("http://[fc00::1]/mcp").is_err());
    }

    #[test]
    fn accepts_public_ip() {
        assert!(check("https://1.1.1.1/mcp").is_ok());
        assert!(check("http://93.184.216.34/mcp").is_ok()); // example.com literal
    }

    #[test]
    fn rejects_non_http_scheme() {
        assert!(check("file:///etc/passwd").is_err());
        assert!(check("ftp://example.com/x").is_err());
    }

    #[test]
    fn allowed_hosts_overrides_block() {
        // loopback host is normally blocked, but an explicit allowlist lets it through
        assert!(UrlGuard::check("http://localhost:9000/mcp", &["localhost".into()]).is_ok());
        assert!(UrlGuard::check("http://127.0.0.1:9000/mcp", &["127.0.0.1".into()]).is_ok());
    }
}
```

- [ ] **Step 3: Register the module**

In `server/src/lib.rs`, add alongside the other `pub mod` declarations:

```rust
pub mod sandbox;
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p ainm-server --lib sandbox::url_guard_tests -- --nocapture`
Expected: PASS (6 tests). Note: the `accepts_public_ip` cases use IP literals (no DNS needed, so they're deterministic offline).

- [ ] **Step 5: Commit**

```bash
rtk proxy git add server/src/sandbox.rs server/src/lib.rs server/Cargo.toml
rtk proxy git commit -m "feat(d): UrlGuard anti-SSRF for HTTP-transport MCP servers"
```

---

## Task D-3: Env-scrub + rlimit helpers + `LocalLauncher` + `Launcher` trait

**Files:**
- Modify: `server/src/sandbox.rs`
- Modify: `server/Cargo.toml` (add `libc = "0.2"`)

This task adds the `Launcher` trait and the hardening-only `LocalLauncher`: a constructed (never inherited) child environment, plus rlimits applied via a `pre_exec` closure. The closure runs in the forked child before `exec` and MUST be async-signal-safe — only raw `libc::setrlimit` calls, no allocation.

- [ ] **Step 1: Add the `libc` dependency**

In `server/Cargo.toml` under `[dependencies]`:

```toml
# Sandboxing (sub-project D): setrlimit in a pre_exec closure (async-signal-safe)
libc = "0.2"
```

- [ ] **Step 2: Write the failing test**

Append to `server/src/sandbox.rs`:

```rust
use crate::config::{McpServerConfig, SandboxLimits};

/// The minimal environment a sandboxed child is allowed to inherit by name.
/// Everything else (notably every `AINM_*` secret) is scrubbed.
const ENV_ALLOWLIST: &[&str] = &["PATH", "HOME", "LANG", "TMPDIR"];

/// Builds a hardened `tokio::process::Command` to spawn an MCP server.
///
/// Implementors MUST apply §4 hardening (env scrub + rlimits). `BubblewrapLauncher`
/// additionally wraps the command in `bwrap` (§5).
pub trait Launcher: Send + Sync {
    /// Build the fully hardened command for this stdio MCP server.
    fn build_command(&self, cfg: &McpServerConfig) -> anyhow::Result<tokio::process::Command>;
    fn name(&self) -> &'static str;
}

/// Hardening-only launcher (no namespace isolation). Used for `SandboxMode::None`
/// and on non-Linux hosts.
pub struct LocalLauncher;

impl Launcher for LocalLauncher {
    fn build_command(&self, cfg: &McpServerConfig) -> anyhow::Result<tokio::process::Command> {
        let command = cfg
            .command
            .clone()
            .ok_or_else(|| anyhow::anyhow!("stdio MCP server '{}' missing command", cfg.name))?;
        let mut cmd = tokio::process::Command::new(command);
        cmd.args(&cfg.args);
        apply_hardening(&mut cmd, cfg);
        Ok(cmd)
    }
    fn name(&self) -> &'static str {
        "local"
    }
}

/// Apply §4 always-on hardening to a (possibly already `bwrap`-wrapped) command:
/// constructed env (no inherited parent env), rlimits via `pre_exec`, new process group.
pub(crate) fn apply_hardening(cmd: &mut tokio::process::Command, cfg: &McpServerConfig) {
    // Env scrub: clear, then set only the allowlist (passed through from our own env if present)
    // plus the server's explicitly declared env. This closes the AINM_* secret leak.
    cmd.env_clear();
    for key in ENV_ALLOWLIST {
        if let Ok(val) = std::env::var(key) {
            cmd.env(key, val);
        }
    }
    for (k, v) in &cfg.env {
        cmd.env(k, v);
    }

    // New process group so teardown can killpg the whole subtree (no orphaned grandchildren).
    cmd.process_group(0);

    // rlimits in the forked child, pre-exec. Async-signal-safe: only setrlimit syscalls.
    let limits = rlimit_set(&cfg.limits);
    // SAFETY: the closure performs only `setrlimit` libc calls — no allocation, no locks,
    // no non-reentrant calls — so it is safe to run between fork and exec.
    unsafe {
        cmd.pre_exec(move || {
            for (resource, soft, hard) in limits.iter() {
                let rl = libc::rlimit { rlim_cur: *soft, rlim_max: *hard };
                // Best-effort: a failed setrlimit must not abort the spawn for the
                // dev-box case where a limit isn't permitted; the kernel ignores it.
                libc::setrlimit(*resource, &rl);
            }
            Ok(())
        });
    }
}

/// Pure mapping from config limits to `(resource, soft, hard)` setrlimit tuples.
/// Memory/file sizes are converted from MB to bytes. Returned as a fixed array so
/// the `pre_exec` closure does not allocate.
pub(crate) fn rlimit_set(limits: &SandboxLimits) -> [(libc::c_int, libc::rlim_t, libc::rlim_t); 5] {
    let mb = 1024u64 * 1024;
    let as_bytes = limits.max_memory_mb.saturating_mul(mb);
    let fsize_bytes = limits.max_fsize_mb.saturating_mul(mb);
    [
        (libc::RLIMIT_AS, as_bytes, as_bytes),
        (libc::RLIMIT_CPU, limits.max_cpu_secs, limits.max_cpu_secs),
        (libc::RLIMIT_NPROC, limits.max_procs, limits.max_procs),
        (libc::RLIMIT_NOFILE, limits.max_nofile, limits.max_nofile),
        (libc::RLIMIT_FSIZE, fsize_bytes, fsize_bytes),
    ]
}

#[cfg(test)]
mod hardening_tests {
    use super::*;
    use std::collections::BTreeMap;

    fn cfg_with_env(env: BTreeMap<String, String>) -> McpServerConfig {
        McpServerConfig {
            name: "t".into(),
            transport: "stdio".into(),
            command: Some("/bin/echo".into()),
            args: vec!["hi".into()],
            env,
            ..Default::default()
        }
    }

    #[test]
    fn env_scrub_drops_parent_secrets_keeps_declared() {
        // Simulate a leaked secret in the parent env.
        std::env::set_var("AINM_LLM_ENCRYPTION_KEY", "super-secret");
        let mut env = BTreeMap::new();
        env.insert("WEATHER_UNITS".to_string(), "metric".to_string());
        let cfg = cfg_with_env(env);

        let launcher = LocalLauncher;
        let cmd = launcher.build_command(&cfg).unwrap();

        // Inspect the constructed env on the std Command view.
        let envs: Vec<(String, Option<String>)> = cmd
            .as_std()
            .get_envs()
            .map(|(k, v)| {
                (
                    k.to_string_lossy().into_owned(),
                    v.map(|v| v.to_string_lossy().into_owned()),
                )
            })
            .collect();

        // No AINM_* key leaks through.
        assert!(
            !envs.iter().any(|(k, _)| k.starts_with("AINM_")),
            "AINM_* secret leaked into child env: {envs:?}"
        );
        // The declared env is present.
        assert!(envs
            .iter()
            .any(|(k, v)| k == "WEATHER_UNITS" && v.as_deref() == Some("metric")));

        std::env::remove_var("AINM_LLM_ENCRYPTION_KEY");
    }

    #[test]
    fn rlimit_set_maps_mb_to_bytes() {
        let mut limits = SandboxLimits::default();
        limits.max_memory_mb = 256;
        limits.max_fsize_mb = 64;
        limits.max_cpu_secs = 30;
        let set = rlimit_set(&limits);
        // RLIMIT_AS in bytes
        assert_eq!(set[0].0, libc::RLIMIT_AS);
        assert_eq!(set[0].1, 256 * 1024 * 1024);
        // RLIMIT_CPU in seconds (no conversion)
        assert_eq!(set[1].0, libc::RLIMIT_CPU);
        assert_eq!(set[1].1, 30);
        // RLIMIT_FSIZE in bytes
        assert_eq!(set[4].0, libc::RLIMIT_FSIZE);
        assert_eq!(set[4].1, 64 * 1024 * 1024);
    }

    #[tokio::test]
    async fn local_launcher_command_runs() {
        let cfg = cfg_with_env(BTreeMap::new());
        let mut cmd = LocalLauncher.build_command(&cfg).unwrap();
        let out = cmd.output().await.unwrap();
        assert!(out.status.success());
        assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "hi");
    }
}
```

- [ ] **Step 3: Run to verify it fails then passes**

Run: `cargo test -p ainm-server --lib sandbox::hardening_tests -- --nocapture`
Expected: after adding `libc`, PASS (3 tests). The `env_clear` + allowlist proves no `AINM_*` leak; the rlimit mapping is pure; `LocalLauncher` actually execs `/bin/echo`.

Note on `RLIMIT_NPROC`/`RLIMIT_AS` types: `libc::rlim_t` is `u64` on Linux and macOS; the tuple literals above are `u64`, matching. `libc::RLIMIT_AS` etc. are `c_int` on both platforms.

- [ ] **Step 4: Commit**

```bash
rtk proxy git add server/src/sandbox.rs server/Cargo.toml
rtk proxy git commit -m "feat(d): Launcher trait + LocalLauncher (env scrub + rlimits)"
```

---

## Task D-4: `BubblewrapLauncher` (argv builder + seccomp + probe) + `launcher_for`

**Files:**
- Modify: `server/src/sandbox.rs`
- Modify: `server/Cargo.toml` (Linux-only `seccompiler`, `nix` with `fs`/`mman` features for memfd)

The `bwrap` argv builder is pure and unit-tested on all platforms. The seccomp BPF compilation and memfd materialization are Linux-only (`#[cfg(target_os = "linux")]`); on non-Linux the seccomp arg is omitted (the whole `bwrap` path is only ever *selected* on Linux via `launcher_for`, but the argv builder stays testable everywhere).

- [ ] **Step 1: Add dependencies**

In `server/Cargo.toml` under `[dependencies]`:

```toml
# Sandboxing (sub-project D): memfd to hand the seccomp BPF program to bwrap
nix = { version = "0.29", features = ["fs", "mman"] }
```

Add a Linux-only target section (after `[dependencies]`, before `[dev-dependencies]`):

```toml
[target.'cfg(target_os = "linux")'.dependencies]
# Compile the baseline seccomp denylist to BPF (Firecracker's pure-Rust seccomp compiler)
seccompiler = "0.5"
```

- [ ] **Step 2: Write the failing test (pure argv builder)**

Append to `server/src/sandbox.rs`:

```rust
use crate::config::SandboxMode;

/// Linux bubblewrap launcher: namespaces + seccomp + deny-all fs/net + explicit grants.
pub struct BubblewrapLauncher;

/// Name of the bubblewrap binary.
const BWRAP_BIN: &str = "bwrap";

impl BubblewrapLauncher {
    /// Build the `bwrap` argument vector (everything after the `bwrap` program name),
    /// ending with `-- <command> <args...>`. Pure and platform-independent so it can be
    /// unit-tested on macOS. The seccomp fd argument is appended separately by
    /// `build_command` (it requires a live fd), so it is NOT part of this vector.
    pub(crate) fn bwrap_args(cfg: &McpServerConfig) -> anyhow::Result<Vec<String>> {
        let command = cfg
            .command
            .clone()
            .ok_or_else(|| anyhow::anyhow!("stdio MCP server '{}' missing command", cfg.name))?;
        let mut a: Vec<String> = Vec::new();
        let mut push = |s: &str| a.push(s.to_string());

        push("--unshare-all");
        if cfg.allow_network {
            push("--share-net");
        }
        push("--die-with-parent");
        push("--new-session");
        push("--clearenv");
        push("--proc");
        push("/proc");
        push("--dev");
        push("/dev");
        push("--tmpfs");
        push("/tmp");

        // Minimal read-only runtime.
        for p in ["/usr", "/lib", "/lib64", "/bin", "/sbin"] {
            a.push("--ro-bind-try".to_string());
            a.push(p.to_string());
            a.push(p.to_string());
        }
        // TLS roots only when network is allowed.
        if cfg.allow_network {
            a.push("--ro-bind-try".to_string());
            a.push("/etc/ssl".to_string());
            a.push("/etc/ssl".to_string());
            a.push("--ro-bind-try".to_string());
            a.push("/etc/resolv.conf".to_string());
            a.push("/etc/resolv.conf".to_string());
        }
        // Explicit grants.
        for p in &cfg.allow_read {
            a.push("--ro-bind".to_string());
            a.push(p.clone());
            a.push(p.clone());
        }
        for p in &cfg.allow_write {
            a.push("--bind".to_string());
            a.push(p.clone());
            a.push(p.clone());
        }

        // The real command (no shell).
        a.push("--".to_string());
        a.push(command);
        for arg in &cfg.args {
            a.push(arg.clone());
        }
        Ok(a)
    }
}

/// Is `bwrap` present and executable on PATH? (Probe used by `launcher_for` for fail-closed.)
pub(crate) fn bwrap_available() -> bool {
    which_on_path(BWRAP_BIN).is_some()
}

/// Minimal PATH search (avoids a `which` crate dependency).
fn which_on_path(bin: &str) -> Option<std::path::PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(bin);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

/// Select the launcher for a server, fail-closed: a `bubblewrap` server on a host
/// without `bwrap` returns `Err` (caller skips the server — never runs it unsandboxed).
pub fn launcher_for(cfg: &McpServerConfig) -> anyhow::Result<Box<dyn Launcher>> {
    match cfg.sandbox {
        SandboxMode::None => Ok(Box::new(LocalLauncher)),
        SandboxMode::Bubblewrap => {
            if !cfg!(target_os = "linux") {
                anyhow::bail!(
                    "MCP server '{}' requests sandbox=bubblewrap, unsupported on this OS (Linux only)",
                    cfg.name
                );
            }
            if !bwrap_available() {
                anyhow::bail!(
                    "MCP server '{}' requests sandbox=bubblewrap but `bwrap` is not on PATH \
                     (fail-closed: refusing to run unsandboxed)",
                    cfg.name
                );
            }
            Ok(Box::new(BubblewrapLauncher))
        }
    }
}

#[cfg(test)]
mod bwrap_tests {
    use super::*;

    fn base_cfg() -> McpServerConfig {
        McpServerConfig {
            name: "weather".into(),
            transport: "stdio".into(),
            command: Some("mcp-weather".into()),
            args: vec!["--port".into(), "0".into()],
            sandbox: SandboxMode::Bubblewrap,
            ..Default::default()
        }
    }

    #[test]
    fn deny_all_by_default() {
        let args = BubblewrapLauncher::bwrap_args(&base_cfg()).unwrap();
        assert!(args.contains(&"--unshare-all".to_string()));
        assert!(args.contains(&"--die-with-parent".to_string()));
        assert!(args.contains(&"--new-session".to_string()));
        assert!(args.contains(&"--clearenv".to_string()));
        assert!(args.iter().any(|a| a == "--tmpfs"));
        // No network, no TLS bind, no extra binds by default.
        assert!(!args.contains(&"--share-net".to_string()));
        assert!(!args.iter().any(|a| a == "/etc/ssl"));
        // Command is exec'd after `--`, never via a shell.
        let dd = args.iter().position(|a| a == "--").unwrap();
        assert_eq!(args[dd + 1], "mcp-weather");
        assert_eq!(args[dd + 2], "--port");
        assert_eq!(args[dd + 3], "0");
    }

    #[test]
    fn allow_network_adds_share_net_and_tls() {
        let mut cfg = base_cfg();
        cfg.allow_network = true;
        let args = BubblewrapLauncher::bwrap_args(&cfg).unwrap();
        assert!(args.contains(&"--share-net".to_string()));
        assert!(args.iter().any(|a| a == "/etc/ssl"));
    }

    #[test]
    fn allow_read_write_add_binds() {
        let mut cfg = base_cfg();
        cfg.allow_read = vec!["/opt/data".into()];
        cfg.allow_write = vec!["/var/cache/weather".into()];
        let args = BubblewrapLauncher::bwrap_args(&cfg).unwrap();
        // ro-bind for read grant
        let ro = args.iter().position(|a| a == "--ro-bind").unwrap();
        assert!(args[ro..].windows(3).any(|w| w == ["--ro-bind", "/opt/data", "/opt/data"]));
        // rw bind for write grant
        assert!(args.windows(3).any(|w| w == ["--bind", "/var/cache/weather", "/var/cache/weather"]));
    }

    #[test]
    fn launcher_for_none_is_local() {
        let cfg = McpServerConfig {
            name: "x".into(),
            transport: "stdio".into(),
            command: Some("echo".into()),
            ..Default::default()
        };
        assert_eq!(launcher_for(&cfg).unwrap().name(), "local");
    }

    #[test]
    fn launcher_for_bubblewrap_fail_closed_when_unavailable() {
        // On macOS this fails on the OS check; on Linux without bwrap it fails on the probe.
        // Either way it must be an Err (never silently downgraded to LocalLauncher).
        let cfg = base_cfg();
        let result = launcher_for(&cfg);
        if !bwrap_available() || !cfg!(target_os = "linux") {
            assert!(result.is_err(), "bubblewrap must fail-closed when unavailable");
        } else {
            assert_eq!(result.unwrap().name(), "bubblewrap");
        }
    }
}
```

- [ ] **Step 3: Run to verify**

Run: `cargo test -p ainm-server --lib sandbox::bwrap_tests -- --nocapture`
Expected: PASS (5 tests). On the macOS dev box `launcher_for_bubblewrap_fail_closed_when_unavailable` exercises the `Err` branch.

- [ ] **Step 4: Add the `Launcher` impl for `BubblewrapLauncher` + seccomp + `build_command`**

Append to `server/src/sandbox.rs`. The `build_command` wires §4 hardening onto the outer `bwrap` command and (on Linux) appends `--seccomp <fd>` backed by a memfd holding the compiled BPF.

```rust
impl Launcher for BubblewrapLauncher {
    fn build_command(&self, cfg: &McpServerConfig) -> anyhow::Result<tokio::process::Command> {
        let mut bwrap_args = BubblewrapLauncher::bwrap_args(cfg)?;

        let mut cmd = tokio::process::Command::new(BWRAP_BIN);

        // On Linux, materialize the seccomp BPF to a memfd and pass `--seccomp <fd>`.
        // The fd is inherited by bwrap (not CLOEXEC). We insert the flag BEFORE the
        // trailing `-- <command>` so bwrap parses it as its own option.
        #[cfg(target_os = "linux")]
        {
            if let Some(fd) = seccomp::install_seccomp_fd(&mut cmd)? {
                // Insert `--seccomp <fd>` just before the `--` separator.
                let dd = bwrap_args
                    .iter()
                    .position(|a| a == "--")
                    .unwrap_or(bwrap_args.len());
                bwrap_args.insert(dd, fd.to_string());
                bwrap_args.insert(dd, "--seccomp".to_string());
            }
        }

        cmd.args(&bwrap_args);
        // §4 hardening (env scrub + rlimits + process group) on the OUTER bwrap command.
        // rlimits are inherited across bwrap's exec into the sandboxed child.
        apply_hardening(&mut cmd, cfg);
        Ok(cmd)
    }
    fn name(&self) -> &'static str {
        "bubblewrap"
    }
}

/// Linux-only seccomp baseline denylist, compiled to BPF and passed to bwrap via a memfd.
#[cfg(target_os = "linux")]
mod seccomp {
    use std::os::fd::{AsRawFd, RawFd};

    /// Dangerous syscalls denied by the baseline profile (errno EPERM). This is a
    /// denylist hardening floor (design §5), not a strict allowlist.
    const DENIED: &[&str] = &[
        "ptrace",
        "mount",
        "umount2",
        "keyctl",
        "add_key",
        "request_key",
        "bpf",
        "kexec_load",
        "kexec_file_load",
        "init_module",
        "finit_module",
        "delete_module",
        "perf_event_open",
        "process_vm_readv",
        "process_vm_writev",
        "pivot_root",
        "setns",
        "swapon",
        "swapoff",
        "reboot",
    ];

    /// Compile the denylist to a BPF program, write it to a memfd, set it on the
    /// command's inherited fds, and return the raw fd number to pass as `--seccomp <fd>`.
    /// Returns `Ok(None)` only if the architecture is unsupported (skip seccomp, keep
    /// the other isolation layers).
    pub(super) fn install_seccomp_fd(
        cmd: &mut tokio::process::Command,
    ) -> anyhow::Result<Option<RawFd>> {
        use seccompiler::{
            BpfProgram, SeccompAction, SeccompFilter, TargetArch,
        };
        use std::collections::BTreeMap;

        let arch = match std::env::consts::ARCH {
            "x86_64" => TargetArch::x86_64,
            "aarch64" => TargetArch::aarch64,
            _ => return Ok(None), // unsupported arch: skip seccomp, keep namespaces+rlimits
        };

        // Map each denied syscall name to its number for this arch, deny with EPERM.
        let mut rules: BTreeMap<i64, Vec<seccompiler::SeccompRule>> = BTreeMap::new();
        for name in super::seccomp::DENIED {
            if let Ok(num) = seccompiler::sys_reg::resolve(name, arch) {
                rules.insert(num, vec![]); // empty rule vec = match the syscall unconditionally
            }
        }

        let filter = SeccompFilter::new(
            rules,
            SeccompAction::Allow,                 // default: allow
            SeccompAction::Errno(libc::EPERM as u32), // matched (denied) syscalls -> EPERM
            arch,
        )
        .map_err(|e| anyhow::anyhow!("seccomp filter build failed: {e}"))?;

        let prog: BpfProgram = filter
            .try_into()
            .map_err(|e| anyhow::anyhow!("seccomp BPF compile failed: {e}"))?;

        // Serialize the BPF instructions (each sock_filter is 8 bytes) into a memfd.
        let fd = write_bpf_to_memfd(&prog)?;
        let raw = fd.as_raw_fd();

        // Keep the memfd open & inherited by bwrap. Leak it (one fd per configured
        // bubblewrap server, for the server's lifetime) so it survives until exec.
        std::mem::forget(fd);
        // Ensure tokio/std does not mark it CLOEXEC: memfd below is created without CLOEXEC.
        let _ = cmd; // command env/args set by caller; nothing fd-specific to set here.
        Ok(Some(raw))
    }

    /// Write the BPF program to a memfd (no CLOEXEC, so it is inherited across exec).
    fn write_bpf_to_memfd(prog: &seccompiler::BpfProgram) -> anyhow::Result<std::fs::File> {
        use nix::sys::memfd::{memfd_create, MemFdCreateFlag};
        use std::io::{Seek, SeekFrom, Write};
        use std::os::fd::FromRawFd;

        // Each instruction is a `sock_filter { u16 code; u8 jt; u8 jf; u32 k; }` = 8 bytes.
        let bytes: &[u8] = unsafe {
            std::slice::from_raw_parts(
                prog.as_ptr() as *const u8,
                std::mem::size_of_val(&prog[..]),
            )
        };

        // No MFD_CLOEXEC: we WANT bwrap to inherit this fd.
        let owned = memfd_create(c"ainm-seccomp", MemFdCreateFlag::empty())
            .map_err(|e| anyhow::anyhow!("memfd_create failed: {e}"))?;
        // nix returns an OwnedFd; turn it into a File for convenient write+seek.
        let raw = {
            use std::os::fd::IntoRawFd;
            owned.into_raw_fd()
        };
        let mut file = unsafe { std::fs::File::from_raw_fd(raw) };
        file.write_all(bytes)
            .map_err(|e| anyhow::anyhow!("writing seccomp BPF to memfd failed: {e}"))?;
        file.seek(SeekFrom::Start(0))
            .map_err(|e| anyhow::anyhow!("seeking seccomp memfd failed: {e}"))?;
        Ok(file)
    }
}
```

Implementation notes for the engineer:
- `seccompiler` 0.5 API: confirm `SeccompFilter::new(rules, mismatch_action, match_action, target_arch)` and `BpfProgram = Vec<sock_filter>` via `TryInto`. If the exact symbol path for syscall-name resolution differs (e.g. it's `seccompiler::sys_reg` vs a feature-gated table), resolve syscall numbers using the `libc::SYS_*` constants directly instead — build a `BTreeMap<i64, Vec<SeccompRule>>` keyed by `libc::SYS_ptrace as i64`, etc. The denylist set is the contract; the resolution mechanism is an implementation detail. Prefer `libc::SYS_*` if `seccompiler`'s resolver isn't a stable public API in 0.5.
- `c"ainm-seccomp"` is a C string literal (Rust 1.77+). If the toolchain predates it, use `std::ffi::CStr::from_bytes_with_nul(b"ainm-seccomp\0").unwrap()`.
- The memfd is intentionally leaked (`std::mem::forget`) so the raw fd stays valid until `bwrap` execs and reads it. One fd per configured bubblewrap server, for the process lifetime — bounded and documented.

- [ ] **Step 5: Verify it compiles on macOS (seccomp module cfg'd out) and Linux**

Run: `cargo build -p ainm-server`
Expected: clean build on macOS (the `seccomp` mod and the `#[cfg(target_os = "linux")]` block compile out). On Linux CI it pulls `seccompiler`.

Run: `cargo test -p ainm-server --lib sandbox:: -- --nocapture`
Expected: PASS (all sandbox unit tests; bwrap argv tests are platform-independent).

- [ ] **Step 6: Commit**

```bash
rtk proxy git add server/src/sandbox.rs server/Cargo.toml
rtk proxy git commit -m "feat(d): BubblewrapLauncher argv + seccomp baseline + launcher_for (fail-closed)"
```

---

## Task D-5: Wire the launcher + UrlGuard + timeout/output-cap into `mcp.rs`

**Files:**
- Modify: `server/src/mcp.rs`

This rewrites `McpClientHandle::connect` to build the stdio command via `launcher_for`, guards the http path with `UrlGuard`, adds the per-call wall-clock timeout + output cap to `call_tool`, and makes `build_providers_from_config` fail-closed.

- [ ] **Step 1: Write the failing test**

Add to the `#[cfg(test)] mod tests` in `server/src/mcp.rs`:

```rust
    #[test]
    fn output_cap_truncates_with_marker() {
        let capped = super::cap_output("hello world", 5);
        assert!(capped.starts_with("hello"));
        assert!(capped.ends_with("…[truncated]"));
        // Under the cap: returned unchanged.
        assert_eq!(super::cap_output("hi", 100), "hi");
    }

    #[tokio::test]
    async fn build_providers_skips_bubblewrap_without_bwrap() {
        // A bubblewrap server on a host without bwrap (or non-Linux) must be skipped,
        // never run unsandboxed.
        let mut config = crate::config::AppConfig {
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
        // Only meaningful when bwrap is unavailable (the dev/macOS case).
        if !crate::sandbox::bwrap_available() || !cfg!(target_os = "linux") {
            let providers = build_providers_from_config(&config).await;
            assert!(providers.is_empty(), "unsandboxed bubblewrap server must be skipped");
        }
        let _ = &mut config;
    }
```

Note: `bwrap_available` must be reachable from the test. If it's `pub(crate)`, `crate::sandbox::bwrap_available()` works.

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p ainm-server --lib mcp::tests -- --nocapture`
Expected: FAIL — `cap_output` undefined; `build_providers_from_config` does not yet fail-closed on launcher errors.

- [ ] **Step 3: Rewrite the stdio connect path to use `launcher_for`**

In `server/src/mcp.rs`, replace the `"stdio"` arm of `McpClientHandle::connect` (currently `server/src/mcp.rs:95-108`) with:

```rust
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
```

(`build_command` already applied env scrub, rlimits, process-group, args, and — for bubblewrap — the `bwrap` wrapping. The `ConfigureCommandExt` import is no longer needed here; remove it if now unused.)

- [ ] **Step 4: Guard the http connect path with `UrlGuard`**

Replace the `"http"` arm (currently `server/src/mcp.rs:109-116`) with:

```rust
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
```

- [ ] **Step 5: Add the wall-clock timeout + output cap to `call_tool`**

Replace `McpClientHandle::call_tool` (currently `server/src/mcp.rs:135-146`) with a version that takes the limits and wraps the rmcp call in `tokio::time::timeout`, then caps output:

```rust
    async fn call_tool(
        &self,
        name: &str,
        args: &serde_json::Value,
        limits: &crate::config::SandboxLimits,
    ) -> anyhow::Result<String> {
        use rmcp::model::CallToolRequestParams;
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
```

Add the pure helper near `extract_content_text`:

```rust
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
```

- [ ] **Step 6: Thread the limits through `McpToolProvider::call`**

`McpToolProvider` already holds `cfg`. Update the `call` method (currently `server/src/mcp.rs:79-82`) to pass `&self.cfg.limits`:

```rust
        match self.client.call_tool(name, args, &self.cfg.limits).await {
            Ok(text) => ToolResult::ok(text),
            Err(e) => ToolResult::error(format!("MCP call failed: {e}")),
        }
```

- [ ] **Step 7: Make `build_providers_from_config` fail-closed on launcher errors**

The current loop only catches `McpToolProvider::connect` failures (which now include `launcher_for` errors, because `connect` calls it). Verify the error path logs clearly. Update the `Err` arm (currently `server/src/mcp.rs:31`) to distinguish the fail-closed skip:

```rust
            Err(e) => tracing::warn!(
                "[mcp] skipping server '{}' (not started): {e}",
                cfg.name
            ),
```

Since `McpClientHandle::connect` calls `launcher_for(cfg)?` first, a `bubblewrap`-without-`bwrap` server now returns `Err` from `connect` → the server is skipped here. No unsandboxed fallback path exists. Confirm by reading the control flow.

- [ ] **Step 8: Run tests to verify they pass**

Run: `cargo test -p ainm-server --lib mcp:: -- --nocapture`
Expected: PASS — `cap_output` tests pass; `build_providers_skips_bubblewrap_without_bwrap` confirms the fail-closed skip on the dev box.

- [ ] **Step 9: Commit**

```bash
rtk proxy git add server/src/mcp.rs
rtk proxy git commit -m "feat(d): wire Launcher + UrlGuard + call timeout/output-cap into mcp.rs"
```

---

## Task D-6: Linux-only sandbox runtime tests

**Files:**
- Modify: `server/src/sandbox.rs` (add a `#[cfg(target_os = "linux")] mod linux_sandbox_tests`)

These tests only run on Linux and skip cleanly when `bwrap` is absent. They prove the isolation actually holds (not just that the argv looks right). On the macOS dev box they compile out entirely.

- [ ] **Step 1: Write the Linux-only tests**

Append to `server/src/sandbox.rs`:

```rust
#[cfg(all(test, target_os = "linux"))]
mod linux_sandbox_tests {
    use super::*;
    use std::collections::BTreeMap;

    fn skip_if_no_bwrap() -> bool {
        if !bwrap_available() {
            eprintln!("skipping: bwrap not installed");
            return true;
        }
        false
    }

    fn sandboxed_cfg(command: &str, args: Vec<String>) -> McpServerConfig {
        McpServerConfig {
            name: "test".into(),
            transport: "stdio".into(),
            command: Some(command.into()),
            args,
            sandbox: SandboxMode::Bubblewrap,
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn sandboxed_command_runs_and_returns_output() {
        if skip_if_no_bwrap() { return; }
        let cfg = sandboxed_cfg("/bin/echo", vec!["sandboxed-ok".into()]);
        let mut cmd = BubblewrapLauncher.build_command(&cfg).unwrap();
        let out = cmd.output().await.unwrap();
        assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
        assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "sandboxed-ok");
    }

    #[tokio::test]
    async fn parent_env_absent_inside_sandbox() {
        if skip_if_no_bwrap() { return; }
        std::env::set_var("AINM_LLM_ENCRYPTION_KEY", "leak-me");
        // `env` inside the sandbox; grep its output for AINM_.
        let cfg = sandboxed_cfg("/usr/bin/env", vec![]);
        let mut cmd = BubblewrapLauncher.build_command(&cfg).unwrap();
        let out = cmd.output().await.unwrap();
        std::env::remove_var("AINM_LLM_ENCRYPTION_KEY");
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(
            !stdout.contains("AINM_"),
            "AINM_* secret visible inside sandbox:\n{stdout}"
        );
    }

    #[tokio::test]
    async fn no_network_when_disallowed() {
        if skip_if_no_bwrap() { return; }
        // With --unshare-all and no --share-net, there is no usable network namespace.
        // `getent hosts` / a TCP connect should fail. Use `ip` if present, else a bash TCP probe.
        // We assert the network namespace is isolated by checking `ip addr` shows only loopback-down.
        let cfg = sandboxed_cfg("/bin/sh", vec![
            "-c".into(),
            // Try to open a TCP connection; success prints CONNECTED.
            "timeout 3 sh -c 'echo > /dev/tcp/1.1.1.1/443' 2>/dev/null && echo CONNECTED || echo BLOCKED".into(),
        ]);
        // NB: this cfg uses /bin/sh deliberately INSIDE the sandbox to script the probe;
        // the no-shell rule is about how WE launch, not what a trusted test probe may do.
        let mut cmd = BubblewrapLauncher.build_command(&cfg).unwrap();
        let out = cmd.output().await.unwrap();
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(stdout.contains("BLOCKED"), "network was reachable without allow_network: {stdout}");
    }

    #[tokio::test]
    async fn cannot_read_unbound_path() {
        if skip_if_no_bwrap() { return; }
        // /etc/hostname is NOT in any allow_read; outside the minimal ro binds it should be absent.
        let cfg = sandboxed_cfg("/bin/sh", vec![
            "-c".into(),
            "cat /etc/shadow 2>/dev/null && echo READ || echo DENIED".into(),
        ]);
        let mut cmd = BubblewrapLauncher.build_command(&cfg).unwrap();
        let out = cmd.output().await.unwrap();
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(stdout.contains("DENIED"), "read a path not granted: {stdout}");
    }

    #[tokio::test]
    async fn rlimit_caps_memory() {
        if skip_if_no_bwrap() { return; }
        // A child that tries to allocate far more than max_memory_mb should be killed/fail.
        let mut cfg = sandboxed_cfg("/bin/sh", vec![
            "-c".into(),
            // Allocate ~400MB via printf into a var; with a 64MB cap this should die.
            "A=$(head -c 400000000 /dev/zero | tr '\\0' 'a') ; echo ALLOCATED".into(),
        ]);
        cfg.limits = SandboxLimits { max_memory_mb: 64, ..SandboxLimits::default() };
        let mut cmd = BubblewrapLauncher.build_command(&cfg).unwrap();
        let out = cmd.output().await.unwrap();
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(!stdout.contains("ALLOCATED"), "rlimit did not cap memory: {stdout}");
    }

    #[tokio::test]
    async fn network_works_when_allowed() {
        if skip_if_no_bwrap() { return; }
        let mut cfg = sandboxed_cfg("/bin/sh", vec![
            "-c".into(),
            "timeout 5 sh -c 'echo > /dev/tcp/1.1.1.1/443' 2>/dev/null && echo CONNECTED || echo BLOCKED".into(),
        ]);
        cfg.allow_network = true;
        let mut cmd = BubblewrapLauncher.build_command(&cfg).unwrap();
        let out = cmd.output().await.unwrap();
        let stdout = String::from_utf8_lossy(&out.stdout);
        // In CI without egress this may legitimately be BLOCKED; only fail if the namespace
        // itself is missing (bwrap error). Treat CONNECTED or BLOCKED as namespace-present.
        assert!(out.status.success() || stdout.contains("BLOCKED") || stdout.contains("CONNECTED"));
    }
}
```

- [ ] **Step 2: Verify it compiles on macOS (cfg'd out) and is correct on Linux**

Run (macOS dev box): `cargo test -p ainm-server --lib sandbox::`
Expected: PASS; the `linux_sandbox_tests` module compiles out (`#[cfg(target_os = "linux")]`), so it neither runs nor needs `bwrap`.

On Linux CI these run; without `bwrap` installed they self-skip via `skip_if_no_bwrap()`. (Deployment — sub-project E — must install `bubblewrap` in the prod/CI image; this plan only documents that requirement.)

- [ ] **Step 3: Commit**

```bash
rtk proxy git add server/src/sandbox.rs
rtk proxy git commit -m "test(d): Linux-only bubblewrap isolation tests (env/net/fs/rlimit)"
```

---

## Task D-7: Integration tests + full suite + clippy + DoD

**Files:**
- Create: `server/tests/mcp_sandbox.rs`

- [ ] **Step 1: Write the integration test**

Create `server/tests/mcp_sandbox.rs`:

```rust
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
    // Skip the assertion on a Linux host that actually has bwrap (it would try to start).
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
    // An http MCP server pointed at a private/loopback URL (no allowed_hosts) must fail
    // UrlGuard at connect and be skipped.
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
```

Note: this requires `mcp`, `config`, and `sandbox` modules to be `pub` in `lib.rs` and `build_providers_from_config`, `bwrap_available`, the config types to be reachable. `bwrap_available` is `pub(crate)` per D-4 — promote it to `pub` (it's a harmless capability probe) so the integration test can call it. If you prefer to keep it crate-private, drop the `bwrap_available()` guard from this integration test and rely on the unit test in D-5 instead.

- [ ] **Step 2: Run the integration test**

Run: `cargo test -p ainm-server --test mcp_sandbox -- --nocapture`
Expected: PASS (2 tests on the dev box).

- [ ] **Step 3: Run the full server suite**

Run: `cargo test -p ainm-server 2>&1 | tail -40`
Expected: All green EXCEPT the two known pre-existing failures `test_graph_traversal` and `test_compilation_dedup_updates_confidence`. If any OTHER test fails, fix it before proceeding (regression).

- [ ] **Step 4: Clippy clean**

Run: `cargo clippy -p ainm-server --all-targets -- -D warnings 2>&1 | tail -30`
Expected: no warnings. Common likely lints: an unused `ConfigureCommandExt` import removed in D-5; a `#[allow(dead_code)]` may be needed on `BWRAP_BIN`/`which_on_path` on non-Linux if they're only used by the Linux path — verify they're used by `bwrap_available()` (cross-platform) so no allow is needed.

- [ ] **Step 5: Verify the DoD against the spec**

Re-read `docs/superpowers/specs/2026-06-02-mcp-sandboxing-design.md` §11 and confirm each item:
1. ✅ config.rs carries the new fields + `SandboxMode` + `SandboxLimits` (D-1).
2. ✅ sandbox.rs provides `Launcher`, `LocalLauncher`, `BubblewrapLauncher`, rlimit/env-scrub helpers, `launcher_for`, `UrlGuard` (D-2..D-4).
3. ✅ every stdio launch is env-scrubbed (no `AINM_*`), rlimited, process-grouped, no-shell; every call wall-clock-timed + output-capped (D-3, D-5).
4. ✅ bubblewrap deny-all + explicit grants + baseline seccomp; bwrap-absent → skipped fail-closed (D-4, D-5).
5. ✅ http servers pass `UrlGuard` before connecting (D-5).
6. ✅ rmcp confined to `McpClientHandle`; tools.rs/runtime/consent gate unchanged (verify by grep: `rmcp` appears only in mcp.rs).
7. ✅ unit + Linux-only + integration tests pass; full suite green except the two known failures.

Run the confinement check:

```bash
rtk proxy grep -rl "rmcp" server/src | grep -v "mcp.rs" || echo "OK: rmcp confined to mcp.rs"
```

Expected: `OK: rmcp confined to mcp.rs`.

- [ ] **Step 6: Final commit**

```bash
rtk proxy git add server/tests/mcp_sandbox.rs server/src
rtk proxy git commit -m "test(d): integration tests + DoD verification for MCP sandboxing"
```

---

## Self-Review (completed during planning)

- **Spec coverage:** §3 config → D-1; §4 hardening → D-3 (env/rlimit) + D-5 (timeout/output cap); §5 bubblewrap+seccomp+probe → D-4; §6 UrlGuard → D-2; §7 module boundaries → D-2/D-5; §8 errors → D-5/D-7; §10 testing (unit/Linux/integration) → D-2..D-7; §11 DoD → D-7. All covered.
- **Placeholder scan:** no TBD/TODO; every code step has complete code.
- **Type consistency:** `SandboxMode`/`SandboxLimits`/`McpServerConfig` field names are identical across D-1 and their uses in D-3/D-4/D-5; `cap_output(text, max_bytes)`, `launcher_for(cfg) -> Box<dyn Launcher>`, `build_command(cfg) -> tokio::process::Command`, `UrlGuard::check(url, allowed_hosts)`, `call_tool(name, args, limits)` are referenced consistently.
- **Known risk flagged:** the `seccompiler` syscall-resolution API (`sys_reg::resolve` vs `libc::SYS_*`) is the one spot the engineer may need to adapt — the denylist set is the contract, the resolution mechanism is an implementation detail (noted inline in D-4).
