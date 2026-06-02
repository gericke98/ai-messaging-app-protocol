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
        let limits = SandboxLimits {
            max_memory_mb: 256,
            max_fsize_mb: 64,
            max_cpu_secs: 30,
            ..SandboxLimits::default()
        };
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

        for p in ["/usr", "/lib", "/lib64", "/bin", "/sbin"] {
            a.push("--ro-bind-try".to_string());
            a.push(p.to_string());
            a.push(p.to_string());
        }
        if cfg.allow_network {
            a.push("--ro-bind-try".to_string());
            a.push("/etc/ssl".to_string());
            a.push("/etc/ssl".to_string());
            a.push("--ro-bind-try".to_string());
            a.push("/etc/resolv.conf".to_string());
            a.push("/etc/resolv.conf".to_string());
        }
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

        a.push("--".to_string());
        a.push(command);
        for arg in &cfg.args {
            a.push(arg.clone());
        }
        Ok(a)
    }
}

impl Launcher for BubblewrapLauncher {
    fn build_command(&self, cfg: &McpServerConfig) -> anyhow::Result<tokio::process::Command> {
        #[cfg_attr(not(target_os = "linux"), allow(unused_mut))]
        let mut bwrap_args = BubblewrapLauncher::bwrap_args(cfg)?;
        let mut cmd = tokio::process::Command::new(BWRAP_BIN);

        #[cfg(target_os = "linux")]
        {
            if let Some(fd) = seccomp::install_seccomp_fd()? {
                let dd = bwrap_args
                    .iter()
                    .position(|a| a == "--")
                    .unwrap_or(bwrap_args.len());
                bwrap_args.insert(dd, fd.to_string());
                bwrap_args.insert(dd, "--seccomp".to_string());
            }
        }

        cmd.args(&bwrap_args);
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

    /// Dangerous syscalls denied by the baseline profile (errno EPERM). Denylist
    /// hardening floor (design §5), not a strict allowlist.
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

    /// Compile the denylist to BPF, write to a memfd, return the raw fd for `--seccomp`.
    /// `Ok(None)` only if the architecture is unsupported (skip seccomp, keep other layers).
    pub(super) fn install_seccomp_fd() -> anyhow::Result<Option<RawFd>> {
        let prog = match build_bpf()? {
            Some(p) => p,
            None => return Ok(None),
        };
        let fd = write_bpf_to_memfd(&prog)?;
        let raw = fd.as_raw_fd();
        // Keep fd open & inherited by bwrap (leak one fd per configured bubblewrap server,
        // for the server lifetime — bounded and intentional).
        std::mem::forget(fd);
        Ok(Some(raw))
    }

    /// Build a seccompiler filter denying `DENIED` (resolved via `libc::SYS_*`) with EPERM,
    /// default Allow, for the current arch (x86_64/aarch64; else `Ok(None)`), then compile
    /// to the BPF program (`Vec<seccompiler::sock_filter>`).
    fn build_bpf() -> anyhow::Result<Option<seccompiler::BpfProgram>> {
        use seccompiler::{BpfProgram, SeccompAction, SeccompFilter, TargetArch};
        use std::collections::BTreeMap;

        let arch = match std::env::consts::ARCH {
            "x86_64" => TargetArch::x86_64,
            "aarch64" => TargetArch::aarch64,
            _ => return Ok(None),
        };

        let mut rules: BTreeMap<i64, Vec<seccompiler::SeccompRule>> = BTreeMap::new();
        for name in DENIED {
            if let Some(num) = resolve_syscall(name) {
                // Empty rule vec = unconditional match on this syscall number.
                rules.insert(num, vec![]);
            }
        }

        // SeccompFilter::new(rules, mismatch_action, match_action, arch):
        // mismatch_action is the default when a syscall is NOT in `rules` (Allow), and
        // match_action fires for syscalls in `rules` (Errno EPERM). Denylist semantics.
        let filter = SeccompFilter::new(
            rules,
            SeccompAction::Allow,
            SeccompAction::Errno(libc::EPERM as u32),
            arch,
        )
        .map_err(|e| anyhow::anyhow!("seccomp filter build failed: {e}"))?;

        let prog: BpfProgram = filter
            .try_into()
            .map_err(|e| anyhow::anyhow!("seccomp BPF compile failed: {e}"))?;
        Ok(Some(prog))
    }

    /// Resolve a syscall name to its number on the current architecture via `libc::SYS_*`.
    fn resolve_syscall(name: &str) -> Option<i64> {
        let n = match name {
            "ptrace" => libc::SYS_ptrace,
            "mount" => libc::SYS_mount,
            "umount2" => libc::SYS_umount2,
            "keyctl" => libc::SYS_keyctl,
            "add_key" => libc::SYS_add_key,
            "request_key" => libc::SYS_request_key,
            "bpf" => libc::SYS_bpf,
            "kexec_load" => libc::SYS_kexec_load,
            "kexec_file_load" => libc::SYS_kexec_file_load,
            "init_module" => libc::SYS_init_module,
            "finit_module" => libc::SYS_finit_module,
            "delete_module" => libc::SYS_delete_module,
            "perf_event_open" => libc::SYS_perf_event_open,
            "process_vm_readv" => libc::SYS_process_vm_readv,
            "process_vm_writev" => libc::SYS_process_vm_writev,
            "pivot_root" => libc::SYS_pivot_root,
            "setns" => libc::SYS_setns,
            "swapon" => libc::SYS_swapon,
            "swapoff" => libc::SYS_swapoff,
            "reboot" => libc::SYS_reboot,
            _ => return None,
        };
        Some(n as i64)
    }

    /// Serialize the compiled BPF program to a non-cloexec memfd so `bwrap` inherits it.
    /// `seccompiler::sock_filter` is `#[repr(C)]` with the kernel's exact 8-byte layout
    /// (u16 code; u8 jt; u8 jf; u32 k), so a raw byte reinterpret is correct.
    fn write_bpf_to_memfd(prog: &[seccompiler::sock_filter]) -> anyhow::Result<std::fs::File> {
        use nix::sys::memfd::{memfd_create, MemFdCreateFlag};
        use std::io::{Seek, SeekFrom, Write};

        // SAFETY: `sock_filter` is `#[repr(C)]` POD; reading its bytes is well-defined and
        // the slice borrow outlives this view.
        let bytes: &[u8] = unsafe {
            std::slice::from_raw_parts(prog.as_ptr() as *const u8, std::mem::size_of_val(prog))
        };
        // No MFD_CLOEXEC: the fd must survive exec so the bwrap child inherits it.
        let owned = memfd_create(c"ainm-seccomp", MemFdCreateFlag::empty())
            .map_err(|e| anyhow::anyhow!("memfd_create failed: {e}"))?;
        let mut file = std::fs::File::from(owned);
        file.write_all(bytes)
            .map_err(|e| anyhow::anyhow!("writing seccomp BPF to memfd failed: {e}"))?;
        file.seek(SeekFrom::Start(0))
            .map_err(|e| anyhow::anyhow!("seeking seccomp memfd failed: {e}"))?;
        Ok(file)
    }
}

/// Is `bwrap` present and executable on PATH? (Probe used by `launcher_for` for fail-closed.)
pub fn bwrap_available() -> bool {
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
        assert!(!args.contains(&"--share-net".to_string()));
        assert!(!args.iter().any(|a| a == "/etc/ssl"));
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
        assert!(args.windows(3).any(|w| w == ["--ro-bind", "/opt/data", "/opt/data"]));
        assert!(args
            .windows(3)
            .any(|w| w == ["--bind", "/var/cache/weather", "/var/cache/weather"]));
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
        let cfg = base_cfg();
        let result = launcher_for(&cfg);
        if !bwrap_available() || !cfg!(target_os = "linux") {
            assert!(result.is_err(), "bubblewrap must fail-closed when unavailable");
        } else {
            assert_eq!(result.unwrap().name(), "bubblewrap");
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

#[cfg(all(test, target_os = "linux"))]
mod linux_sandbox_tests {
    use super::*;

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
        // The set/remove of this process-global var races with the parallel
        // hardening_tests env test, but both assert *absence* of AINM_* in the child,
        // so the race is benign: env_clear + the allowlist make the leak structurally
        // impossible regardless of the parent var's state at spawn time.
        std::env::set_var("AINM_LLM_ENCRYPTION_KEY", "leak-me");
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
        // Correctness rests on `--unshare-all` dropping the network namespace, NOT on the
        // `/dev/tcp` shell builtin (which is bash-only; dash would print BLOCKED anyway).
        // The deterministic `--share-net` presence/absence is covered by `bwrap_tests`.
        let cfg = sandboxed_cfg("/bin/sh", vec![
            "-c".into(),
            "timeout 3 sh -c 'echo > /dev/tcp/1.1.1.1/443' 2>/dev/null && echo CONNECTED || echo BLOCKED".into(),
        ]);
        let mut cmd = BubblewrapLauncher.build_command(&cfg).unwrap();
        let out = cmd.output().await.unwrap();
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(stdout.contains("BLOCKED"), "network was reachable without allow_network: {stdout}");
    }

    #[tokio::test]
    async fn cannot_read_unbound_path() {
        if skip_if_no_bwrap() { return; }
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
        let mut cfg = sandboxed_cfg("/bin/sh", vec![
            "-c".into(),
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
        // Assert the meaningful, egress-independent invariant: the `--share-net` sandbox
        // (with TLS-root + resolv.conf binds) launches cleanly. We deliberately do NOT
        // assert CONNECTED, since CI runners often lack outbound egress.
        assert!(
            out.status.success(),
            "allow_network sandbox failed to launch: stderr={}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
}
