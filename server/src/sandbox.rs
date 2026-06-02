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
