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

/// Build a canonical Agent Card for a live connector agent from its `ainm.initialize` params.
///
/// `address` is the agent's AINM address (`name@host`); `init_params` is the `params` object of
/// the initialize request. Card fields come from `params.card` (sent by the connector SDK);
/// transport capabilities come from `params.capabilities`. All keys camelCase. Pure: no I/O.
///
/// Robust to partial/absent input: a missing `card` yields a card built from capabilities +
/// defaults (name = address local-part, description = "{name} agent"). Malformed tools/skills are
/// dropped (logged), never panicking and never failing the caller.
pub fn build_connector_card(address: &str, init_params: &Value) -> Value {
    let local = address.split('@').next().unwrap_or(address);
    let host = address.split('@').nth(1).unwrap_or(address);
    let card = init_params.get("card");
    let caps = init_params.get("capabilities");

    let name = card
        .and_then(|c| c.get("name"))
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or(local)
        .to_string();
    let description = card
        .and_then(|c| c.get("description"))
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("{name} agent"));
    let version = card
        .and_then(|c| c.get("version"))
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("0.1.0")
        .to_string();

    let streaming = caps
        .and_then(|c| c.get("streaming"))
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let sub_threads = caps
        .and_then(|c| c.get("subThreads"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    // The schema's AINMExtensions.contentTypes requires minItems:1, so an empty or absent
    // array is normalized to the universal default ["text"] (an empty card would be invalid).
    let content_types: Vec<Value> = caps
        .and_then(|c| c.get("contentTypes"))
        .and_then(|v| v.as_array())
        .cloned()
        .filter(|a| !a.is_empty())
        .unwrap_or_else(|| vec![json!("text")]);

    let skills = normalize_skills(card.and_then(|c| c.get("skills")));
    let tools = normalize_tools(card.and_then(|c| c.get("tools")));

    json!({
        "name": name,
        "description": description,
        "url": format!("https://{host}"),
        "version": version,
        "protocolVersion": "0.1.0",
        "capabilities": { "streaming": streaming, "pushNotifications": false },
        "skills": skills,
        "ainm": {
            "protocolVersion": "0.1.0",
            "address": address,
            "type": "agent",
            "contentTypes": content_types,
            "streaming": { "supported": streaming, "parallelBlocks": false, "preferredChunkSize": "token" },
            "threading": { "subThreads": sub_threads, "maxConcurrentThreads": null },
            "memory": { "archivalSupported": false, "contextFramePreferred": false },
            "tools": tools
        }
    })
}

/// Normalize `card.skills` into valid `AgentSkill` objects. The schema requires id/name/description;
/// derive a slug `id` from `name` when absent. Drop entries lacking name or description.
fn normalize_skills(skills: Option<&Value>) -> Vec<Value> {
    let Some(arr) = skills.and_then(|v| v.as_array()) else { return vec![] };
    arr.iter()
        .filter_map(|s| {
            let name = s.get("name").and_then(|v| v.as_str()).filter(|x| !x.is_empty())?;
            let description = s.get("description").and_then(|v| v.as_str()).filter(|x| !x.is_empty())?;
            let id = s
                .get("id")
                .and_then(|v| v.as_str())
                .filter(|x| !x.is_empty())
                .map(|x| x.to_string())
                .unwrap_or_else(|| slug(name));
            if id.is_empty() {
                tracing::warn!("[connector-card] dropping skill `{name}`: derived id is empty");
                return None;
            }
            Some(json!({ "id": id, "name": name, "description": description }))
        })
        .collect()
}

/// Normalize `card.tools` into valid `AgentTool` objects. The schema requires
/// name/description/inputSchema, with `inputSchema` a JSON object. Drop entries whose `parameters`
/// is missing or not an object (logged), or lacking name/description.
fn normalize_tools(tools: Option<&Value>) -> Vec<Value> {
    let Some(arr) = tools.and_then(|v| v.as_array()) else { return vec![] };
    arr.iter()
        .filter_map(|t| {
            let name = t.get("name").and_then(|v| v.as_str()).filter(|x| !x.is_empty())?;
            let description = t.get("description").and_then(|v| v.as_str()).filter(|x| !x.is_empty())?;
            match t.get("parameters") {
                Some(p) if p.is_object() => {
                    Some(json!({ "name": name, "description": description, "inputSchema": p.clone() }))
                }
                _ => {
                    tracing::warn!("[connector-card] dropping tool `{name}`: parameters missing or not an object");
                    None
                }
            }
        })
        .collect()
}

/// Lowercase alphanumeric slug with `-` separators (for deriving a skill id from its name).
fn slug(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev_dash = false;
    for ch in s.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            prev_dash = false;
        } else if !prev_dash && !out.is_empty() {
            out.push('-');
            prev_dash = true;
        }
    }
    out.trim_end_matches('-').to_string()
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

    #[test]
    fn build_connector_card_full_params_is_canonical() {
        let params = json!({
            "identity": "coder@h.test",
            "capabilities": { "streaming": true, "subThreads": true, "contentTypes": ["text", "code"] },
            "card": {
                "name": "Coder",
                "description": "Writes and reviews code.",
                "version": "2.1.0",
                "skills": [{ "name": "Review", "description": "Reviews diffs." }],
                "tools": [{ "name": "run_tests", "description": "Runs tests.", "parameters": { "type": "object" } }]
            }
        });
        let card = build_connector_card("coder@h.test", &params);

        assert_eq!(card["name"], "Coder");
        assert_eq!(card["description"], "Writes and reviews code.");
        assert_eq!(card["url"], "https://h.test");
        assert_eq!(card["version"], "2.1.0");
        assert_eq!(card["protocolVersion"], "0.1.0");
        assert_eq!(card["capabilities"]["streaming"], true);
        assert_eq!(card["capabilities"]["pushNotifications"], false);
        assert_eq!(card["skills"][0]["id"], "review");
        assert_eq!(card["skills"][0]["name"], "Review");
        let ainm = &card["ainm"];
        assert_eq!(ainm["address"], "coder@h.test");
        assert_eq!(ainm["type"], "agent");
        assert_eq!(ainm["contentTypes"], json!(["text", "code"]));
        assert_eq!(ainm["streaming"]["supported"], true);
        assert_eq!(ainm["streaming"]["parallelBlocks"], false);
        assert_eq!(ainm["streaming"]["preferredChunkSize"], "token");
        assert_eq!(ainm["threading"]["subThreads"], true);
        assert!(ainm["threading"]["maxConcurrentThreads"].is_null());
        assert_eq!(ainm["memory"]["archivalSupported"], false);
        assert_eq!(ainm["tools"][0]["name"], "run_tests");
        assert_eq!(ainm["tools"][0]["inputSchema"], json!({ "type": "object" }));

        let s = serde_json::to_string(&card).unwrap();
        for snake in ["push_notifications", "protocol_version", "content_types", "input_schema", "sub_threads"] {
            assert!(!s.contains(snake), "snake_case key `{snake}` leaked: {s}");
        }
    }

    #[test]
    fn build_connector_card_drops_malformed_tool() {
        let params = json!({
            "capabilities": { "contentTypes": ["text"] },
            "card": {
                "name": "X",
                "tools": [
                    { "name": "ok", "description": "good", "parameters": { "type": "object" } },
                    { "name": "bad", "description": "no params" },
                    { "name": "bad2", "description": "string params", "parameters": "nope" }
                ]
            }
        });
        let card = build_connector_card("x@h.test", &params);
        let tools = card["ainm"]["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 1, "only the well-formed tool survives");
        assert_eq!(tools[0]["name"], "ok");
    }

    #[test]
    fn build_connector_card_defaults_without_card_object() {
        let params = json!({ "capabilities": { "contentTypes": ["text"] } });
        let card = build_connector_card("solver@h.test", &params);
        assert_eq!(card["name"], "solver");
        assert_eq!(card["description"], "solver agent");
        assert_eq!(card["version"], "0.1.0");
        assert_eq!(card["ainm"]["tools"], json!([]));
        assert_eq!(card["skills"], json!([]));
        assert_eq!(card["ainm"]["contentTypes"], json!(["text"]));
    }

    #[test]
    fn build_connector_card_drops_skill_with_empty_slug() {
        // A skill whose name has no alphanumerics slugs to "" → schema-invalid id → dropped.
        let params = json!({
            "card": { "name": "X", "skills": [
                { "name": "!!!", "description": "all symbols" },
                { "name": "Good Skill", "description": "fine" }
            ] }
        });
        let card = build_connector_card("x@h.test", &params);
        let skills = card["skills"].as_array().unwrap();
        assert_eq!(skills.len(), 1, "the all-symbol-name skill must be dropped");
        assert_eq!(skills[0]["id"], "good-skill");
    }
}
