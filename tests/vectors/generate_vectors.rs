// Standalone Rust script for generating cross-platform test vectors.
// Run from the server/ directory: cargo run --example generate_vectors
//
// Or compile directly:
//   rustc generate_vectors.rs --edition 2021 (requires ring, serde_json, etc.)
//
// For simplicity, this is designed to run via the server project.
// Add to server/Cargo.toml:
//   [[example]]
//   name = "generate_vectors"
//   path = "../tests/vectors/generate_vectors.rs"

use ring::signature::{Ed25519KeyPair, KeyPair};
use serde_json::json;
use serde_json_canonicalizer::to_string as canonicalize;

fn base64url_encode(bytes: &[u8]) -> String {
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use base64::Engine;
    URL_SAFE_NO_PAD.encode(bytes)
}

fn main() {
    // Use a fixed 32-byte seed for deterministic test vectors.
    // This is the RFC 8032 Section 7.1 Test Vector 1 seed.
    // Note: ring's Ed25519 derivation may produce a different public key
    // than the RFC 8032 test vector because ring hashes the seed internally.
    // What matters is that the seed -> keypair -> sign -> verify pipeline is
    // consistent across platforms using the same derivation.
    let private_seed =
        hex::decode("9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60").unwrap();

    let key_pair = Ed25519KeyPair::from_seed_unchecked(&private_seed).unwrap();
    let public_key = key_pair.public_key().as_ref();

    println!(
        "Private key seed (base64url): {}",
        base64url_encode(&private_seed)
    );
    println!("Public key (base64url): {}", base64url_encode(public_key));

    // --- JCS Vectors ---
    let jcs_vectors = vec![
        ("simple_sorted_keys", json!({"b": 2, "a": 1})),
        ("nested_objects", json!({"z": {"b": 2, "a": 1}, "a": 0})),
        (
            "boolean_null_types",
            json!({"active": true, "deleted": false, "meta": null}),
        ),
        ("string_escaping", json!({"text": "line1\nline2\ttab"})),
        (
            "message_envelope_shape",
            json!({
                "id": "019580a1-7b3e-7000-8000-abcdef123456",
                "from": "alice@messaging.example.com",
                "thread_id": "019580a0-1234-7000-8000-000000000001",
                "timestamp": "2026-03-28T12:34:56.789Z",
                "type": "message",
                "content": [{"type": "text", "version": "1.0", "source": "user", "text": "Hello", "fallback": "Hello"}]
            }),
        ),
    ];

    let mut jcs_out = Vec::new();
    for (name, input) in &jcs_vectors {
        let canonical = canonicalize(input).unwrap();
        jcs_out.push(json!({
            "name": name,
            "input": input,
            "expected_canonical": canonical
        }));
    }

    let jcs_doc = json!({
        "description": "JCS canonicalization test vectors for cross-platform validation",
        "spec_reference": "RFC 8785 (JSON Canonicalization Scheme)",
        "vectors": jcs_out
    });
    println!("\n=== jcs_vectors.json ===");
    println!("{}", serde_json::to_string_pretty(&jcs_doc).unwrap());

    // --- Signing Vectors ---
    // Test Vector 1: Simple message from security.md Section 10.2
    let envelope1 = json!({
        "id": "01935801-0000-7000-8000-000000000001",
        "from": "test@example.com",
        "thread_id": "01935800-0000-7000-8000-000000000001",
        "timestamp": "2026-01-01T00:00:00.000Z",
        "type": "message",
        "content": [{"type": "text", "version": "1.0", "source": "user", "text": "Hello, world!", "fallback": "Hello, world!"}]
    });

    let canonical1 = canonicalize(&envelope1).unwrap();
    let sig1 = key_pair.sign(canonical1.as_bytes());

    // Test Vector 2: Unicode content from security.md Section 10.3
    let envelope2 = json!({
        "id": "01935801-0000-7000-8000-000000000002",
        "from": "test@example.com",
        "thread_id": "01935800-0000-7000-8000-000000000001",
        "timestamp": "2026-01-01T00:00:01.000Z",
        "type": "message",
        "content": [{"type": "text", "version": "1.0", "source": "user", "text": "Hola mundo! \u{1F30D} \u{4F60}\u{597D}\u{4E16}\u{754C}", "fallback": "Hola mundo!"}]
    });

    let canonical2 = canonicalize(&envelope2).unwrap();
    let sig2 = key_pair.sign(canonical2.as_bytes());

    // Test Vector 3: Multiple content blocks from security.md Section 10.4
    let envelope3 = json!({
        "id": "01935801-0000-7000-8000-000000000003",
        "from": "weather-bot@agents.example.com",
        "thread_id": "01935800-0000-7000-8000-000000000001",
        "timestamp": "2026-01-01T00:00:02.000Z",
        "type": "message",
        "content": [
            {"type": "text", "version": "1.0", "source": "agent", "text": "Here is the weather data:", "fallback": "Here is the weather data:"},
            {"type": "code", "version": "1.0", "source": "agent", "language": "json", "code": "{\"city\":\"Madrid\",\"temp_c\":22}", "fallback": "Madrid: 22C"}
        ]
    });

    let canonical3 = canonicalize(&envelope3).unwrap();
    let sig3 = key_pair.sign(canonical3.as_bytes());

    let signing_doc = json!({
        "description": "Ed25519 signing test vectors for cross-platform validation",
        "spec_reference": "security.md Section 10 + RFC 8032",
        "note": "Private key included ONLY for testing. Never include real private keys in test vectors.",
        "key_pair": {
            "algorithm": "Ed25519",
            "private_key_seed_hex": "9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60",
            "private_key_base64url": base64url_encode(&private_seed),
            "public_key_hex": hex::encode(public_key),
            "public_key_base64url": base64url_encode(public_key),
            "source": "RFC 8032 Section 7.1 Test Vector 1 key pair"
        },
        "vectors": [
            {
                "name": "simple_text_message",
                "description": "security.md Section 10.2",
                "envelope_without_signature": envelope1,
                "expected_canonical": canonical1,
                "expected_signature_base64url": base64url_encode(sig1.as_ref())
            },
            {
                "name": "unicode_content",
                "description": "security.md Section 10.3",
                "envelope_without_signature": envelope2,
                "expected_canonical": canonical2,
                "expected_signature_base64url": base64url_encode(sig2.as_ref())
            },
            {
                "name": "multiple_content_blocks",
                "description": "security.md Section 10.4",
                "envelope_without_signature": envelope3,
                "expected_canonical": canonical3,
                "expected_signature_base64url": base64url_encode(sig3.as_ref())
            }
        ]
    });

    println!("\n=== signing_vectors.json ===");
    println!("{}", serde_json::to_string_pretty(&signing_doc).unwrap());
}
