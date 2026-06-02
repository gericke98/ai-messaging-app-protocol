# MCP Sandboxing & Isolation — Design (sub-project D, v1)

**Date:** 2026-06-02
**Status:** Approved design
**Author:** brainstorming session (Santiago + Claude)
**Milestone context:** Sub-project D of the hosted multi-tenant runtime initiative. Decomposition:
A = Accounts & identity binding (shipped) → B = Hosted agent runtime (shipped) → C = Per-user agent
credentials & quota (shipped) → **D = Isolation/sandboxing (this doc)** → E = Deployment/HA → F = Onboarding UX.

## 1. Goal & threat model

Sub-project B shipped a curated/trusted MCP bridge behind a `ToolProvider` trait. Today an MCP server is
spawned with **zero isolation**: `McpClientHandle::connect` builds a `tokio::process::Command` and hands it to
rmcp's `TokioChildProcess::new`. The child **inherits the server's full environment** (`AINM_LLM__API_KEY`,
`AINM_DATABASE__URL`, `AINM_LLM_ENCRYPTION_KEY`, `AINM_TOKEN_SECRET`), has **no execution timeout, no output
cap**, and runs with **full ambient privilege** in the server's own process namespace. HTTP-transport servers
connect to an unvalidated URL (SSRF surface).

D adds a **pluggable sandbox layer** behind a new `Launcher` trait:

- **Always-on portable hardening** on every stdio launch (env scrub, rlimits, process-group kill, no-shell,
  per-call timeout, output cap) — ships and tests on macOS + Linux.
- **An optional Linux `BubblewrapLauncher`** giving real filesystem / network / syscall isolation with a
  **deny-all default + explicit per-server grants**.
- **A `UrlGuard`** for the HTTP transport (anti-SSRF).

**Threat model (v1):** buggy, supply-chain-compromised, or misconfigured **operator** MCP servers. There is no
user-supplied-MCP registration path yet (config-only), so the sandbox is **defense-in-depth below** B's
`required_capability` consent gate, not the sole authorization layer. The sandbox establishes the *mechanism* so
that untrusted/user-supplied MCP can be enabled safely later.

**Non-goal (v1):** containing a determined kernel-0-day attacker. Bubblewrap is shared-kernel (namespaces + seccomp
+ rlimits), strong against the v1 threat model but weaker than a microVM. Stronger backends (E2B/Firecracker,
container, gVisor) are **future implementations of the same `Launcher` trait** (§9) — the seam is the point.

## 2. Decisions locked (brainstorming)

- **Scope = `Launcher` trait + always-on hardening + a Linux `BubblewrapLauncher`.** E2B and container/gVisor are
  future backends behind the same trait (not built in v1). (E2B was evaluated and judged an awkward *primary* fit
  for a self-hosted Rust server — cloud dependency conflicts with the "we host the runtime" moat, no Rust SDK,
  heavy self-host; it remains a sensible *optional* backend for a hosted deployment.)
- **Sandbox default posture = deny-all + explicit grants.** A bubblewrap-sandboxed server gets no network and no
  filesystem beyond a private tmpfs scratch (+ read-only runtime libs); `allow_network` / `allow_read` /
  `allow_write` opt specific access back in.
- **`sandbox` field defaults to `"none"`** (backward-compat for existing curated configs), but **hardening (§4)
  applies to every server regardless**. Non-first-party servers should set `sandbox = "bubblewrap"`.
- **Fail-closed:** a server configured `sandbox = "bubblewrap"` on a host without `bwrap` **does not start** — never
  silently downgraded to no isolation.
- **Resource limits via `setrlimit` (`pre_exec`), not cgroups, in v1.** rlimits are dependency-light and need no
  cgroup delegation; cgroup-based limits defer to E (deployment).
- **Seccomp = a baseline denylist** profile in v1 (block the most dangerous syscalls), not a strict per-server
  allowlist.
- **No shell, ever** — commands are exec'd as argv (`bwrap ... -- <cmd> <args>`), never `sh -c`.
- **rmcp stays confined to `McpClientHandle`** (B's isolation seam) — the `Launcher` only *constructs* the
  `tokio::process::Command`; `McpClientHandle` still owns the rmcp `serve`/`call`.

## 3. Config schema (`McpServerConfig` extension)

New optional, serde-defaulted fields (existing TOML/env configs keep loading unchanged):

```rust
// server/src/config.rs
#[derive(Debug, Clone, serde::Deserialize, Default)]
pub struct McpServerConfig {
    pub name: String,
    pub transport: String,                 // "stdio" | "http"
    #[serde(default)] pub command: Option<String>,
    #[serde(default)] pub args: Vec<String>,
    #[serde(default)] pub url: Option<String>,
    pub required_capability: String,
    // --- D additions ---
    #[serde(default)] pub sandbox: SandboxMode,           // default None
    #[serde(default)] pub allow_network: bool,            // default false (deny)
    #[serde(default)] pub allow_read: Vec<String>,        // ro bind mounts
    #[serde(default)] pub allow_write: Vec<String>,       // rw bind mounts
    #[serde(default)] pub env: std::collections::BTreeMap<String, String>, // explicit child env (TOML inline table: env = { KEY = "val" })
    #[serde(default)] pub allowed_hosts: Vec<String>,     // http transport: SSRF allowlist (hostnames)
    #[serde(default)] pub limits: SandboxLimits,
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum SandboxMode { #[default] None, Bubblewrap }

#[derive(Debug, Clone, serde::Deserialize)]
pub struct SandboxLimits {
    #[serde(default = "default_max_memory_mb")]   pub max_memory_mb: u64,    // RLIMIT_AS
    #[serde(default = "default_max_cpu_secs")]    pub max_cpu_secs: u64,     // RLIMIT_CPU
    #[serde(default = "default_max_procs")]       pub max_procs: u64,        // RLIMIT_NPROC
    #[serde(default = "default_max_nofile")]      pub max_nofile: u64,       // RLIMIT_NOFILE
    #[serde(default = "default_max_fsize_mb")]    pub max_fsize_mb: u64,     // RLIMIT_FSIZE
    #[serde(default = "default_max_output_bytes")] pub max_output_bytes: usize, // call output cap
    #[serde(default = "default_wall_timeout_secs")] pub wall_timeout_secs: u64, // per-call wall clock
}
impl Default for SandboxLimits { /* delegates to the default_* fns */ }
// defaults: mem 512, cpu 30, procs 64, nofile 256, fsize 64, output 1_048_576, wall 60
```

Example:
```toml
[[mcp_servers]]
name = "weather"
transport = "stdio"
command = "uvx"
args = ["mcp-weather"]
required_capability = "patterns"
sandbox = "bubblewrap"
allow_network = true
[mcp_servers.limits]
max_memory_mb = 256
wall_timeout_secs = 30
```

## 4. Always-on hardening (applies to every stdio launch, both launchers)

Built into the `Command` that any `Launcher` produces:

1. **Env scrub.** The child receives a *constructed* environment, never the inherited parent env:
   `Command::env_clear()` then set a minimal allowlist (`PATH`, `HOME`, `LANG`, `TMPDIR`) + the server's declared
   `cfg.env`. This is what closes the `AINM_*` secret-leak. (For the `BubblewrapLauncher`, env is set on the outer
   `bwrap` command; `bwrap` does not add env back.)
2. **rlimits** via `std::os::unix::process::CommandExt::pre_exec` calling `libc::setrlimit` (or the `nix`
   `setrlimit`, already a transitive dep) for `RLIMIT_AS` (`max_memory_mb`), `RLIMIT_CPU` (`max_cpu_secs`),
   `RLIMIT_NPROC` (`max_procs`), `RLIMIT_NOFILE` (`max_nofile`), `RLIMIT_FSIZE` (`max_fsize_mb`). The `pre_exec`
   closure runs in the forked child before exec; it must be async-signal-safe (only `setrlimit` calls, no
   allocation). `pre_exec` is `#[cfg(unix)]`; on non-unix it is a no-op (macOS *is* unix, so rlimits apply on the
   dev box too).
3. **Process-group isolation + kill-on-drop.** `Command::process_group(0)` (new pgid) + rmcp's
   `TokioChildProcess` already `kill_on_drop`; on teardown the runtime kills the whole process group
   (`killpg`) so no orphaned grandchildren survive.
4. **No shell.** The command is exec'd as argv directly. The `command` string is treated as a program name, never
   passed to a shell.
5. **Per-call wall-clock timeout + output cap.** `McpClientHandle::call_tool` wraps the rmcp call in
   `tokio::time::timeout(wall_timeout_secs)`; on elapse it kills the process group and returns a timeout error.
   The flattened tool-result text is truncated to `max_output_bytes` (with a `…[truncated]` marker).

## 5. `BubblewrapLauncher` (Linux filesystem/network/syscall isolation)

`build_command(cfg)` produces a `tokio::process::Command` for `bwrap` wrapping the real command:

```
bwrap \
  --unshare-all \                 # new user/mount/pid/ipc/uts/cgroup[/net] namespaces
  [--share-net] \                 # ONLY if cfg.allow_network
  --die-with-parent \             # kernel kills the sandbox if our process dies
  --new-session \                 # detach controlling tty (no TIOCSTI injection)
  --clearenv \                    # belt-and-suspenders; env then set on the Command
  --proc /proc --dev /dev \
  --tmpfs /tmp \                  # private scratch
  --ro-bind /usr /usr \           # minimal read-only runtime (usr, lib, lib64, bin, etc/ssl certs)
  --ro-bind /lib /lib --ro-bind /lib64 /lib64 --ro-bind /bin /bin \
  --ro-bind /etc/ssl /etc/ssl \   # only when allow_network (TLS roots)
  [--ro-bind <p> <p> for p in cfg.allow_read] \
  [--bind   <p> <p> for p in cfg.allow_write] \
  --seccomp <fd> \                # baseline denylist profile (passed as an inherited fd)
  -- <cfg.command> <cfg.args...>
```

- **Network**: `--unshare-all` drops the net namespace (no connectivity) unless `cfg.allow_network` adds
  `--share-net`. TLS root bind only added when network is allowed.
- **Filesystem**: deny-all except the private `--tmpfs /tmp`, minimal read-only runtime mounts, and the explicit
  `allow_read` (ro) / `allow_write` (rw) binds.
- **Seccomp**: a baseline profile (deny `ptrace`, `mount`, `keyctl`, `add_key`, `bpf`, `clone` with namespace
  flags, `kexec_*`, `*module`, `perf_event_open`, etc.) compiled in code, materialized to a memfd/pipe and passed
  to `bwrap --seccomp <fd>`. v1 is a denylist (not a strict allowlist) — documented as a hardening baseline.
- **rlimits + env scrub (§4)** are set on the *outer* `bwrap` Command (rlimits are inherited across exec into the
  sandboxed child; env is constructed, not inherited).
- **`bwrap` presence probe** (`which`/`Command` check at build) — if absent, `build_command` returns an error and
  `build_providers_from_config` **skips that server with a logged error (fail-closed)**.

The `Launcher` for stdio:
```rust
// server/src/sandbox.rs
pub trait Launcher: Send + Sync {
    /// Build the (fully hardened) command to spawn for this stdio MCP server.
    fn build_command(&self, cfg: &McpServerConfig) -> anyhow::Result<tokio::process::Command>;
    fn name(&self) -> &'static str;
}
pub struct LocalLauncher;       // hardening only (§4), no bwrap wrapping
pub struct BubblewrapLauncher;  // §4 hardening + bwrap wrapping (§5)

/// Pick the launcher for a server; errors (fail-closed) if bubblewrap requested but unavailable.
pub fn launcher_for(cfg: &McpServerConfig) -> anyhow::Result<Box<dyn Launcher>>;
```

## 6. HTTP transport — `UrlGuard`

HTTP-transport servers have no process to sandbox; they get connect-time URL hardening:

```rust
// server/src/sandbox.rs
pub struct UrlGuard;
impl UrlGuard {
    /// Reject loopback / private (RFC1918) / link-local / ULA / unspecified targets unless the
    /// resolved host is in cfg.allowed_hosts. Returns Ok(()) if the URL is safe to connect to.
    pub fn check(url: &str, allowed_hosts: &[String]) -> anyhow::Result<()>;
}
```

- Parse the URL, resolve the host to IPs, and **reject** loopback (`127.0.0.0/8`, `::1`), private
  (`10/8`,`172.16/12`,`192.168/16`), link-local (`169.254/16`, `fe80::/10`), ULA (`fc00::/7`), and unspecified
  (`0.0.0.0`/`::`) — the SSRF set — **unless** the hostname is explicitly listed in `cfg.allowed_hosts`.
- Require `http`/`https` scheme.
- Called in `McpClientHandle::connect` before `StreamableHttpClientTransport::from_uri`.

Note (documented v1 limitation): this is a connect-time check; it does not defend against DNS-rebinding mid-session
(the rmcp HTTP client may re-resolve). Acceptable for the v1 operator-config threat model; a deny-egress network
policy (E) is the durable fix.

## 7. Module boundaries

- **`sandbox.rs`** (new) — the `Launcher` trait + `LocalLauncher` + `BubblewrapLauncher` (argv builder,
  `bwrap`-presence probe, seccomp profile bytes), the rlimit `pre_exec` helper + env-scrub helper, `launcher_for`,
  and `UrlGuard` (URL parse + IP classification). The argv/policy/classification builders are pure and
  unit-testable; the `bwrap` *exec* path is Linux-only (skip-tested on macOS).
- **`mcp.rs`** (modify) — `McpClientHandle::connect`: stdio path calls `launcher_for(cfg)?.build_command(cfg)?` and
  hands the result to `TokioChildProcess::new`; http path calls `UrlGuard::check` before connecting. `call_tool`
  gains the wall-clock timeout + output cap (§4.5). `build_providers_from_config` constructs each provider via
  `launcher_for`, **skipping (logged) any server whose launcher errors** (fail-closed). `rmcp` remains confined
  here.
- **`config.rs`** (modify) — the new `McpServerConfig` fields + `SandboxMode` + `SandboxLimits` with serde
  defaults.

No change to `tools.rs` (`ToolProvider`/`ToolRegistry`), `runtime.rs`/`runtime_server.rs`, or B's consent gate.

## 8. Error handling

- **`bwrap` missing for a `bubblewrap` server** → `launcher_for` errors → `build_providers_from_config` logs and
  skips that server (fail-closed; the agent simply lacks those tools).
- **Sandbox setup / `bwrap` non-zero exit** → the rmcp connect fails → that provider is skipped at startup (logged).
- **Tool call exceeds `wall_timeout_secs`** → kill the process group, return `ToolResult::error("tool timed
  out")` (the registry/runtime already tolerate provider errors — no loop crash).
- **Output over `max_output_bytes`** → truncate with a marker; still returned as a normal result.
- **rlimit kill** (child exceeds mem/cpu) → the child dies; the rmcp call fails → `ToolResult::error`.
- **Env scrub** never logs secret values; only the *names* of allowed vars at debug level.
- **`UrlGuard` rejection** → connect fails → provider skipped (logged with the rejected host, not internal detail).

## 9. Out of scope (v1)

E2B / container (Docker/Podman) / gVisor launchers (defined as future `Launcher` impls — not built); cgroup-based
CPU/memory accounting (rlimits suffice for v1; cgroups need delegation → E); user-supplied MCP registration UX /
endpoint (none exists; D is the mechanism for when it does); a Dockerfile / base image that bundles `bwrap` +
`seccomp` (deployment = E — D documents the requirement: prod images must include `bubblewrap`); macOS *production*
sandboxing (dev falls back to hardening-only, which is expected); DNS-rebinding defense for the HTTP transport
(needs network egress policy → E); strict per-server seccomp allowlists (v1 ships a baseline denylist).

## 10. Testing

**Unit (all platforms, no `bwrap` needed — test the builders/policy):**
- `BubblewrapLauncher` argv builder: deny-all (no `--share-net`, no extra binds) vs `allow_network=true`
  (`--share-net` + TLS bind) vs `allow_read`/`allow_write` (correct `--ro-bind`/`--bind` flags); always includes
  `--unshare-all --die-with-parent --new-session --tmpfs /tmp --seccomp`.
- Env scrub: the constructed env contains only the allowlist + `cfg.env`, and **no** `AINM_*` key (assert by
  building the `Command` and inspecting `get_envs()`).
- rlimit mapping: config limits → the right `(resource, soft, hard)` tuples (test the pure mapping fn).
- `UrlGuard::check`: rejects loopback/private/link-local/ULA/unspecified; accepts a public IP; accepts a
  private/loopback host **iff** in `allowed_hosts`; rejects non-http(s) schemes.
- `launcher_for`: `SandboxMode::None` → `LocalLauncher`; `SandboxMode::Bubblewrap` with the `bwrap`-probe mocked
  absent → `Err` (fail-closed); present → `BubblewrapLauncher`.

**Linux-only (skip on macOS via `#[cfg(target_os = "linux")]` + a `bwrap`-present guard; run in CI):**
- A real `bwrap`-wrapped `printf`/`cat` executes and returns output.
- A sandboxed process **cannot** read a path not in `allow_read` (open fails).
- A sandboxed process with `allow_network=false` **cannot** open a TCP socket (connect fails); with
  `allow_network=true` it can.
- The parent env is **absent** inside the child (spawn `env`/read `/proc/self/environ`, assert no `AINM_*`).
- An rlimit actually caps (e.g. a child that tries to allocate > `max_memory_mb` is killed).

**Integration (`server/tests/`):**
- A `sandbox="bubblewrap"` server configured on a host without `bwrap` is **skipped, not run unsandboxed**
  (`build_providers_from_config` returns no provider for it; logged).
- The existing curated/`MockToolProvider` agent path (B-10) and the consent gate still pass unchanged.

## 11. Definition of done

1. `config.rs` carries the new `McpServerConfig` fields + `SandboxMode` + `SandboxLimits` (serde-defaulted;
   existing configs still load).
2. `sandbox.rs` provides the `Launcher` trait, `LocalLauncher` (hardening-only), `BubblewrapLauncher` (argv +
   seccomp + `bwrap` probe), the rlimit/env-scrub helpers, `launcher_for` (fail-closed), and `UrlGuard`.
3. Every stdio MCP launch is hardened: env-scrubbed (no `AINM_*` leak), rlimited, process-grouped + kill-on-drop,
   no-shell; every call is wall-clock-timed + output-capped.
4. `BubblewrapLauncher` applies deny-all network+filesystem by default with explicit `allow_*` grants and a
   baseline seccomp profile; a `bubblewrap` server without `bwrap` present is skipped (fail-closed), never run
   unsandboxed.
5. HTTP-transport servers pass `UrlGuard` (anti-SSRF) before connecting.
6. `rmcp` remains confined to `McpClientHandle`; `tools.rs`/runtime/consent gate unchanged.
7. Unit tests (all platforms) + Linux-only sandbox tests + integration tests pass; full server suite green except
   the two known pre-existing failures (`test_graph_traversal`, `test_compilation_dedup_updates_confidence`).
