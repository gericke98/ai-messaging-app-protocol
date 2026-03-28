use anyhow::{anyhow, Result};
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use ring::signature::{Ed25519KeyPair, UnparsedPublicKey, ED25519};
use serde_json_canonicalizer::to_string as canonicalize;

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
