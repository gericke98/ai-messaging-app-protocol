use anyhow::{anyhow, Result};
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use ring::signature::{Ed25519KeyPair, UnparsedPublicKey, ED25519};
use serde_json_canonicalizer::to_string as canonicalize;

/// Sign the init payload `"{identity}|{timestamp}"`. Returns (signature_b64url, timestamp).
/// The caller MUST place both the signature and the timestamp in the handshake params so
/// the server can reconstruct and verify the payload (see verify_init).
pub fn sign_init(key_pair: &Ed25519KeyPair, identity: &str) -> Result<(String, String)> {
    let timestamp = crate::protocol::now_rfc3339();
    let payload = format!("{}|{}", identity, timestamp);
    let sig = key_pair.sign(payload.as_bytes());
    Ok((URL_SAFE_NO_PAD.encode(sig.as_ref()), timestamp))
}

/// Sign a message envelope following security.md Section 4:
/// 1. Clone envelope, remove "signature" field
/// 2. Canonicalize with RFC 8785
/// 3. Sign canonical UTF-8 bytes with Ed25519
/// 4. Encode as base64url without padding
/// 5. Set envelope["signature"]
pub fn sign_envelope(
    key_pair: &Ed25519KeyPair,
    envelope: &mut serde_json::Value,
) -> Result<String> {
    // 1. Clone and remove signature for signing input
    let mut signing_input = envelope.clone();
    if let Some(obj) = signing_input.as_object_mut() {
        obj.remove("signature");
    }

    // 2. Canonicalize per RFC 8785
    let canonical = canonicalize(&signing_input)
        .map_err(|e| anyhow!("canonicalization failed: {}", e))?;

    // 3. Sign the canonical UTF-8 bytes
    let sig = key_pair.sign(canonical.as_bytes());

    // 4. Encode as base64url without padding
    let sig_str = URL_SAFE_NO_PAD.encode(sig.as_ref());

    // 5. Set signature on envelope
    if let Some(obj) = envelope.as_object_mut() {
        obj.insert("signature".to_string(), serde_json::Value::String(sig_str.clone()));
    }

    Ok(sig_str)
}

/// Verify a message envelope's signature following security.md Section 5:
/// 1. Clone envelope, extract signature
/// 2. Remove "signature" field
/// 3. Canonicalize
/// 4. Decode signature from base64url
/// 5. Verify with Ed25519 public key
pub fn verify_signature(
    public_key_bytes: &[u8],
    envelope: &serde_json::Value,
) -> Result<bool> {
    // 1. Clone and extract signature
    let mut signing_input = envelope.clone();
    let sig_str = signing_input
        .get("signature")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("missing signature field"))?
        .to_string();

    // 2. Remove signature field
    if let Some(obj) = signing_input.as_object_mut() {
        obj.remove("signature");
    }

    // 3. Canonicalize
    let canonical = canonicalize(&signing_input)
        .map_err(|e| anyhow!("canonicalization failed: {}", e))?;

    // 4. Decode signature
    let sig_bytes = URL_SAFE_NO_PAD
        .decode(&sig_str)
        .map_err(|e| anyhow!("base64url decode failed: {}", e))?;

    // 5. Verify
    let public_key = UnparsedPublicKey::new(&ED25519, public_key_bytes);
    match public_key.verify(canonical.as_bytes(), &sig_bytes) {
        Ok(()) => Ok(true),
        Err(_) => Ok(false),
    }
}

/// Verify a handshake signature against a known public key.
/// Reconstructs `"{identity}|{timestamp}"`, checks the Ed25519 signature, and rejects
/// timestamps outside +/- `max_skew_secs` (replay/clock-skew guard).
pub fn verify_init(
    public_key_bytes: &[u8],
    identity: &str,
    timestamp: &str,
    signature_b64: &str,
    now: chrono::DateTime<chrono::Utc>,
    max_skew_secs: i64,
) -> bool {
    // Freshness: timestamp must parse and be within the window.
    let Ok(ts) = chrono::DateTime::parse_from_rfc3339(timestamp) else { return false; };
    let skew = (now - ts.with_timezone(&chrono::Utc)).num_seconds().abs();
    if skew > max_skew_secs { return false; }

    let Ok(sig_bytes) = URL_SAFE_NO_PAD.decode(signature_b64) else { return false; };
    let payload = format!("{}|{}", identity, timestamp);
    let public_key = UnparsedPublicKey::new(&ED25519, public_key_bytes);
    public_key.verify(payload.as_bytes(), &sig_bytes).is_ok()
}

/// Domain-separation prefix for the auth-session mint challenge. Distinct from the
/// handshake payload (`"{identity}|{timestamp}"`) so signatures cannot be replayed
/// across the two protocols.
///
/// SECURITY INVARIANT: this separator is unforgeable only because `validate_username`
/// (accounts.rs) restricts usernames to `[a-z0-9_]` — so no identity (`username@domain`)
/// can begin with the literal `ainm-auth-token|`. If username rules ever admit `-` or
/// `|`, switch to a length-prefixed/structured payload to preserve domain separation.
const AUTH_CHALLENGE_DOMAIN: &str = "ainm-auth-token";

/// Sign the auth-session mint challenge: `"ainm-auth-token|{identity}|{timestamp}"`.
/// Returns `(signature_b64url, timestamp_rfc3339)`. Reference/test helper mirroring
/// `sign_init` — the server only ever *verifies*; signing happens on the client (iOS).
/// Exercised by the auth-sessions integration tests; allowed-dead in the binary build.
#[allow(dead_code)]
pub fn sign_auth_challenge(key_pair: &Ed25519KeyPair, identity: &str) -> Result<(String, String)> {
    let timestamp = crate::protocol::now_rfc3339();
    let payload = format!("{AUTH_CHALLENGE_DOMAIN}|{identity}|{timestamp}");
    let sig = key_pair.sign(payload.as_bytes());
    Ok((URL_SAFE_NO_PAD.encode(sig.as_ref()), timestamp))
}

/// Verify an auth-session mint challenge signature against a known public key.
/// Reconstructs `"ainm-auth-token|{identity}|{timestamp}"`, checks the Ed25519
/// signature, and rejects timestamps outside +/- `max_skew_secs`.
pub fn verify_auth_challenge(
    public_key_bytes: &[u8],
    identity: &str,
    timestamp: &str,
    signature_b64: &str,
    now: chrono::DateTime<chrono::Utc>,
    max_skew_secs: i64,
) -> bool {
    let Ok(ts) = chrono::DateTime::parse_from_rfc3339(timestamp) else { return false; };
    let skew = (now - ts.with_timezone(&chrono::Utc)).num_seconds().abs();
    if skew > max_skew_secs { return false; }

    let Ok(sig_bytes) = URL_SAFE_NO_PAD.decode(signature_b64) else { return false; };
    let payload = format!("{AUTH_CHALLENGE_DOMAIN}|{identity}|{timestamp}");
    let public_key = UnparsedPublicKey::new(&ED25519, public_key_bytes);
    public_key.verify(payload.as_bytes(), &sig_bytes).is_ok()
}

#[cfg(test)]
mod auth_challenge_tests {
    use super::*;
    use crate::identity::generate_keypair;

    #[test]
    fn accepts_valid_rejects_tamper_stale_and_handshake_payload() {
        let id = generate_keypair("alice@test.local").unwrap();
        let (sig, ts) = sign_auth_challenge(&id.key_pair, "alice@test.local").unwrap();
        let now = chrono::DateTime::parse_from_rfc3339(&ts).unwrap().with_timezone(&chrono::Utc);

        // Valid
        assert!(verify_auth_challenge(&id.public_key_bytes, "alice@test.local", &ts, &sig, now, 300));
        // Tampered identity
        assert!(!verify_auth_challenge(&id.public_key_bytes, "mallory@test.local", &ts, &sig, now, 300));
        // Wrong key
        let other = generate_keypair("bob@test.local").unwrap();
        assert!(!verify_auth_challenge(&other.public_key_bytes, "alice@test.local", &ts, &sig, now, 300));
        // Stale timestamp
        let later = now + chrono::Duration::seconds(10_000);
        assert!(!verify_auth_challenge(&id.public_key_bytes, "alice@test.local", &ts, &sig, later, 300));
        // DOMAIN SEPARATION: a handshake-style signature (verify_init payload, no
        // "ainm-auth-token|" prefix) must NOT verify as an auth challenge.
        let (handshake_sig, hts) = sign_init(&id.key_pair, "alice@test.local").unwrap();
        let hnow = chrono::DateTime::parse_from_rfc3339(&hts).unwrap().with_timezone(&chrono::Utc);
        assert!(!verify_auth_challenge(&id.public_key_bytes, "alice@test.local", &hts, &handshake_sig, hnow, 300));
    }
}

#[cfg(test)]
mod verify_init_tests {
    use super::*;
    use crate::identity::generate_keypair;

    #[test]
    fn accepts_valid_rejects_tampered_and_stale() {
        let id = generate_keypair("alice@test.local").unwrap();
        let (sig, ts) = sign_init(&id.key_pair, "alice@test.local").unwrap();
        let now = chrono::DateTime::parse_from_rfc3339(&ts).unwrap().with_timezone(&chrono::Utc);

        assert!(verify_init(&id.public_key_bytes, "alice@test.local", &ts, &sig, now, 300));
        assert!(!verify_init(&id.public_key_bytes, "mallory@test.local", &ts, &sig, now, 300));
        let other = generate_keypair("bob@test.local").unwrap();
        assert!(!verify_init(&other.public_key_bytes, "alice@test.local", &ts, &sig, now, 300));
        let later = now + chrono::Duration::seconds(10_000);
        assert!(!verify_init(&id.public_key_bytes, "alice@test.local", &ts, &sig, later, 300));
    }
}

/// Sign relay params with server-level Ed25519 key (FAUT-03).
///
/// Similar to `sign_envelope` but uses the `server_signature` field name.
/// The server signature covers the entire relay params object (including the
/// `messages` array), proving the sending server authorized this relay.
///
/// 1. Clone params, remove `server_signature` field from the clone
/// 2. Canonicalize with RFC 8785 (JCS)
/// 3. Sign canonical bytes with Ed25519
/// 4. Encode as base64url (URL_SAFE_NO_PAD)
/// 5. Insert `server_signature` into the original params
pub fn sign_relay_params(
    key_pair: &Ed25519KeyPair,
    params: &mut serde_json::Value,
) -> Result<String> {
    // 1. Clone and remove server_signature for signing input
    let mut signing_input = params.clone();
    if let Some(obj) = signing_input.as_object_mut() {
        obj.remove("server_signature");
    }

    // 2. Canonicalize per RFC 8785
    let canonical = canonicalize(&signing_input)
        .map_err(|e| anyhow!("canonicalization failed: {}", e))?;

    // 3. Sign the canonical UTF-8 bytes
    let sig = key_pair.sign(canonical.as_bytes());

    // 4. Encode as base64url without padding
    let sig_str = URL_SAFE_NO_PAD.encode(sig.as_ref());

    // 5. Insert server_signature into params
    if let Some(obj) = params.as_object_mut() {
        obj.insert(
            "server_signature".to_string(),
            serde_json::Value::String(sig_str.clone()),
        );
    }

    Ok(sig_str)
}

/// Verify a relay params object's server-level signature (FAUT-03).
///
/// Similar to `verify_signature` but uses the `server_signature` field name.
///
/// 1. Clone params, extract `server_signature` value
/// 2. Remove `server_signature` from the clone
/// 3. Canonicalize with RFC 8785
/// 4. Decode signature from base64url
/// 5. Verify with Ed25519 public key
pub fn verify_relay_signature(
    public_key_bytes: &[u8],
    params: &serde_json::Value,
) -> Result<bool> {
    // 1. Clone and extract server_signature
    let mut signing_input = params.clone();
    let sig_str = signing_input
        .get("server_signature")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("missing server_signature field"))?
        .to_string();

    // 2. Remove server_signature field
    if let Some(obj) = signing_input.as_object_mut() {
        obj.remove("server_signature");
    }

    // 3. Canonicalize
    let canonical = canonicalize(&signing_input)
        .map_err(|e| anyhow!("canonicalization failed: {}", e))?;

    // 4. Decode signature
    let sig_bytes = URL_SAFE_NO_PAD
        .decode(&sig_str)
        .map_err(|e| anyhow!("base64url decode failed: {}", e))?;

    // 5. Verify
    let public_key = UnparsedPublicKey::new(&ED25519, public_key_bytes);
    match public_key.verify(canonical.as_bytes(), &sig_bytes) {
        Ok(()) => Ok(true),
        Err(_) => Ok(false),
    }
}
