//! Canonical Agent Card builder for the hosted first-party assistant.
//!
//! Produces a schema-conformant (camelCase, A2A-compatible superset) `AgentCard`
//! JSON value from the assistant's AINM address. Pure: no DB, no async, no I/O.
//! The published JSON Schema (`schemas/agent-card.schema.json`) is authoritative;
//! keys here are camelCase to match it. `tools: []` for v1 — enumerating the live
//! MCP toolset in the card is a future enhancement.

use serde_json::{json, Value};

/// Build the canonical Agent Card for the hosted assistant at `address`
/// (an AINM address, `assistant@<host>`). The card's `url` is derived from the
/// address host (`https://<host>`).
pub fn build_assistant_card(address: &str) -> Value {
    let host = address.split('@').nth(1).unwrap_or(address);
    json!({
        "name": "Assistant",
        "description": "Your home server's built-in AI assistant.",
        "url": format!("https://{host}"),
        "version": "0.1.0",
        "protocolVersion": "0.1.0",
        "capabilities": {
            "streaming": true,
            "pushNotifications": false
        },
        "skills": [],
        "ainm": {
            "protocolVersion": "0.1.0",
            "address": address,
            "type": "agent",
            "contentTypes": ["text", "code"],
            "streaming": {
                "supported": true,
                "parallelBlocks": false,
                "preferredChunkSize": "token"
            },
            "threading": {
                "subThreads": true,
                "maxConcurrentThreads": null
            },
            "memory": {
                "archivalSupported": true,
                "contextFramePreferred": true
            },
            "tools": []
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_assistant_card_is_canonical_camelcase() {
        let card = build_assistant_card("assistant@example.test");

        // Top-level required fields (schema: name, description, url, version,
        // protocolVersion, capabilities, skills, ainm).
        assert_eq!(card["name"], "Assistant");
        assert!(card["description"].as_str().is_some_and(|s| !s.is_empty()));
        assert_eq!(card["url"], "https://example.test");
        assert!(card["version"].as_str().is_some_and(|s| !s.is_empty()));
        assert_eq!(card["protocolVersion"], "0.1.0");
        assert_eq!(card["capabilities"]["streaming"], true);
        assert_eq!(card["capabilities"]["pushNotifications"], false);
        assert!(card["skills"].is_array());

        // AINM extensions (camelCase keys, schema $defs/AINMExtensions).
        let ainm = &card["ainm"];
        assert_eq!(ainm["protocolVersion"], "0.1.0");
        assert_eq!(ainm["address"], "assistant@example.test");
        assert_eq!(ainm["type"], "agent");
        assert_eq!(ainm["contentTypes"], json!(["text", "code"]));
        assert_eq!(ainm["streaming"]["supported"], true);
        assert_eq!(ainm["streaming"]["parallelBlocks"], false);
        assert_eq!(ainm["streaming"]["preferredChunkSize"], "token");
        assert_eq!(ainm["threading"]["subThreads"], true);
        assert!(ainm["threading"]["maxConcurrentThreads"].is_null());
        assert_eq!(ainm["memory"]["archivalSupported"], true);
        assert_eq!(ainm["memory"]["contextFramePreferred"], true);
        assert_eq!(ainm["tools"], json!([]));

        // No snake_case leakage anywhere in the serialized form.
        let s = serde_json::to_string(&card).unwrap();
        for snake in [
            "push_notifications", "protocol_version", "content_types",
            "parallel_blocks", "preferred_chunk_size", "sub_threads",
            "max_concurrent_threads", "archival_supported", "context_frame_preferred",
            "input_schema",
        ] {
            assert!(!s.contains(snake), "card must not contain snake_case key `{snake}`: {s}");
        }
    }

    #[test]
    fn build_assistant_card_address_without_host_falls_back() {
        // Defensive: a malformed address (no `@host`) still yields a valid card.
        let card = build_assistant_card("assistant");
        assert_eq!(card["ainm"]["address"], "assistant");
        assert_eq!(card["url"], "https://assistant");
    }
}
