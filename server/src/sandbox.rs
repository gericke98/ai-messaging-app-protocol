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
            // Canonicalize IPv4-mapped (::ffff:a.b.c.d) addresses back to v4 and re-run
            // the v4 checks — otherwise a mapped loopback/metadata address (e.g.
            // ::ffff:127.0.0.1, ::ffff:169.254.169.254) would slip past the v6 masks.
            // `to_ipv4_mapped` (not `to_ipv4`) leaves `::1` / `::` to the v6 checks below
            // rather than mis-mapping loopback to 0.0.0.1.
            if let Some(mapped) = v6.to_ipv4_mapped() {
                return is_blocked_ip(&IpAddr::V4(mapped));
            }
            v6.is_loopback()
                || v6.is_unspecified()
                // link-local fe80::/10
                || (v6.segments()[0] & 0xffc0) == 0xfe80
                // unique-local fc00::/7
                || (v6.segments()[0] & 0xfe00) == 0xfc00
        }
    }
}

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
    use std::os::unix::process::CommandExt;

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
                let rl = libc::rlimit {
                    rlim_cur: *soft,
                    rlim_max: *hard,
                };
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
        std::env::set_var("AINM_LLM_ENCRYPTION_KEY", "super-secret");
        let mut env = BTreeMap::new();
        env.insert("WEATHER_UNITS".to_string(), "metric".to_string());
        let cfg = cfg_with_env(env);

        let launcher = LocalLauncher;
        let cmd = launcher.build_command(&cfg).unwrap();

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

        assert!(
            !envs.iter().any(|(k, _)| k.starts_with("AINM_")),
            "AINM_* secret leaked into child env: {envs:?}"
        );
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
        assert_eq!(set[0].0, libc::RLIMIT_AS);
        assert_eq!(set[0].1, 256 * 1024 * 1024);
        assert_eq!(set[1].0, libc::RLIMIT_CPU);
        assert_eq!(set[1].1, 30);
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
    fn rejects_ipv4_mapped_v6() {
        // IPv4-mapped IPv6 must not bypass the v4 SSRF checks.
        assert!(check("http://[::ffff:127.0.0.1]:9000/mcp").is_err()); // mapped loopback
        assert!(check("http://[::ffff:169.254.169.254]/meta").is_err()); // mapped metadata
        assert!(check("http://[::ffff:10.0.0.1]/mcp").is_err()); // mapped RFC1918
    }

    #[test]
    fn rejects_non_http_scheme() {
        assert!(check("file:///etc/passwd").is_err());
        assert!(check("ftp://example.com/x").is_err());
    }

    #[test]
    fn allowed_hosts_overrides_block() {
        assert!(UrlGuard::check("http://localhost:9000/mcp", &["localhost".into()]).is_ok());
        assert!(UrlGuard::check("http://127.0.0.1:9000/mcp", &["127.0.0.1".into()]).is_ok());
    }
}
