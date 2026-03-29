//! Federation relay module: inbound relay handler, thread registry, and
//! federation shared state.
//!
//! Implements transport.md Section 5.1-5.4:
//! - Inbound `ainm.relay` JSON-RPC handler with triple-layer validation (FAUT-03)
//! - Thread registry with origin-server authority (RELY-03, D-10/D-11/D-12)
//! - Federated message delivery and fan-out

use crate::protocol;
use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

// ---------------------------------------------------------------------------
// Thread Registry
// ---------------------------------------------------------------------------

/// Metadata for a federated thread.
pub struct ThreadInfo {
    /// The domain that created (and has sequencing authority over) this thread.
    pub origin_domain: String,
    /// Set of server IDs that participate in this thread.
    pub participating_servers: HashSet<String>,
    /// Monotonic sequence counter (only meaningful on the origin server).
    pub sequence_counter: u64,
    /// Maps server_id -> sequence number at which the server joined.
    /// Used for backfill visibility: servers only see messages after their join sequence.
    pub join_sequences: HashMap<String, u64>,
}

/// In-memory registry of federated threads.
///
/// Per D-10: thread IDs encode their origin domain.
/// Per D-11: only the origin server assigns sequence numbers.
/// Per D-12: thread ID format is `{origin-domain}:{uuidv7}`.
#[derive(Clone, Default)]
pub struct ThreadRegistry {
    inner: Arc<Mutex<HashMap<String, ThreadInfo>>>,
}

impl ThreadRegistry {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Create a new thread owned by this server (origin). Per D-10.
    /// Returns the thread ID in `{origin-domain}:{uuidv7}` format.
    pub fn create_thread(&self, origin_domain: &str) -> String {
        let thread_id = format!("{}:{}", origin_domain, uuid::Uuid::now_v7());
        let info = ThreadInfo {
            origin_domain: origin_domain.to_string(),
            participating_servers: HashSet::new(),
            sequence_counter: 0,
            join_sequences: HashMap::new(),
        };
        self.inner.lock().unwrap().insert(thread_id.clone(), info);
        thread_id
    }

    /// Add a remote server to a thread's participant set.
    /// Records the current sequence as the join point for backfill visibility.
    pub fn add_server(&self, thread_id: &str, server_id: &str) {
        if let Some(info) = self.inner.lock().unwrap().get_mut(thread_id) {
            if info.participating_servers.insert(server_id.to_string()) {
                // New server joining: record the current sequence as their join point
                info.join_sequences
                    .insert(server_id.to_string(), info.sequence_counter);
            }
        }
    }

    /// Get the sequence number at which a server joined a thread.
    /// Returns None if the server is not a participant.
    pub fn get_join_sequence(&self, thread_id: &str, server_id: &str) -> Option<u64> {
        self.inner
            .lock()
            .unwrap()
            .get(thread_id)
            .and_then(|info| info.join_sequences.get(server_id).copied())
    }

    /// Check if a server is a participant in a thread.
    pub fn is_participant(&self, thread_id: &str, server_id: &str) -> bool {
        self.inner
            .lock()
            .unwrap()
            .get(thread_id)
            .map(|info| info.participating_servers.contains(server_id))
            .unwrap_or(false)
    }

    /// Get next sequence number (origin only). Per D-11.
    pub fn next_sequence(&self, thread_id: &str) -> Option<u64> {
        let mut map = self.inner.lock().unwrap();
        map.get_mut(thread_id).map(|info| {
            info.sequence_counter += 1;
            info.sequence_counter
        })
    }

    /// Get thread info (origin domain, participating servers).
    pub fn get_info(&self, thread_id: &str) -> Option<(String, HashSet<String>)> {
        self.inner.lock().unwrap().get(thread_id).map(|info| {
            (
                info.origin_domain.clone(),
                info.participating_servers.clone(),
            )
        })
    }

    /// Register a thread we learned about from a relay (not origin).
    pub fn register_remote_thread(&self, thread_id: &str, origin_domain: &str) {
        let mut map = self.inner.lock().unwrap();
        if !map.contains_key(thread_id) {
            map.insert(
                thread_id.to_string(),
                ThreadInfo {
                    origin_domain: origin_domain.to_string(),
                    participating_servers: HashSet::new(),
                    sequence_counter: 0,
                    join_sequences: HashMap::new(),
                },
            );
        }
    }
}

/// Extract the origin domain from a thread ID.
///
/// Thread ID format per D-12: `{origin-domain}:{uuidv7}`
pub fn parse_origin_domain(thread_id: &str) -> Option<&str> {
    thread_id.split_once(':').map(|(domain, _)| domain)
}

// ---------------------------------------------------------------------------
// Federation Shared State
// ---------------------------------------------------------------------------

/// Shared state for the federation subsystem.
///
/// Passed to the relay handler and used by the relay sender.
#[derive(Clone)]
pub struct FederationState {
    /// This server's domain identity (e.g., "server-a.local").
    pub server_id: String,
    /// This server's Ed25519 keypair for signing relay payloads.
    pub server_keypair: Arc<crate::identity::Identity>,
    /// This server's published federation identity.
    pub server_identity: Arc<crate::identity::ServerIdentity>,
    /// Registry of known threads and their origin/participants.
    pub thread_registry: ThreadRegistry,
    /// Store of participant identities (for per-message signature verification).
    pub identity_store: crate::identity::IdentityStore,
    /// Cache of resolved remote server identities.
    pub remote_cache: crate::identity::RemoteServerCache,
    /// Outbound relay sender (set after construction).
    pub relay_sender: Option<Arc<crate::relay::RelaySender>>,
    /// Channel to deliver messages to local participants.
    pub local_delivery_tx: tokio::sync::broadcast::Sender<String>,
    /// Persistent storage layer (redb).
    pub persistence: crate::persistence::PersistenceLayer,
}

// ---------------------------------------------------------------------------
// Relay Handler (Inbound)
// ---------------------------------------------------------------------------

/// Axum POST handler for `POST /ainm/v1/relay`.
///
/// Implements triple-layer validation per FAUT-03:
///   Layer 1: mTLS certificate fingerprint (wired in Plan 04 via middleware)
///   Layer 2: Server-level `server_signature` verification
///   Layer 3: Per-message Ed25519 `signature` verification
///
/// After validation, delivers messages locally and fans out if origin server.
pub async fn relay_handler(
    State(state): State<FederationState>,
    Json(body): Json<serde_json::Value>,
) -> (StatusCode, Json<serde_json::Value>) {
    // 1. Parse JSON-RPC envelope
    let method = body
        .get("method")
        .and_then(|m| m.as_str())
        .unwrap_or("");
    let id = body.get("id").cloned().unwrap_or(serde_json::json!(1));

    match method {
        "ainm.relay" => {} // fall through to relay handling below
        "ainm.backfill" => {
            return handle_backfill(state, id, body).await;
        }
        _ => {
            return (
                StatusCode::OK,
                Json(protocol::new_error_value(id, -32601, "Method not found")),
            );
        }
    }

    // --- ainm.relay handling ---

    let params = match body.get("params") {
        Some(p) => p.clone(),
        None => {
            return (
                StatusCode::OK,
                Json(protocol::new_error_value(id, -32602, "Missing params")),
            )
        }
    };

    let from_server = match params.get("from_server").and_then(|s| s.as_str()) {
        Some(s) => s.to_string(),
        None => {
            return (
                StatusCode::OK,
                Json(protocol::new_error_value(
                    id,
                    -32602,
                    "Missing from_server",
                )),
            )
        }
    };

    // 2. Replay protection (FAUT-05: D-01, D-03, D-04)
    let relay_timestamp = match params.get("relay_timestamp").and_then(|t| t.as_str()) {
        Some(t) => t.to_string(),
        None => {
            return (
                StatusCode::OK,
                Json(protocol::new_error_value(id, -32026, "Missing relay_timestamp")),
            );
        }
    };
    let relay_id = match params.get("relay_id").and_then(|r| r.as_str()) {
        Some(r) => r.to_string(),
        None => {
            return (
                StatusCode::OK,
                Json(protocol::new_error_value(id, -32026, "Missing relay_id")),
            );
        }
    };

    // Check timestamp freshness: reject if older than 5 minutes or more than 1 minute in the future
    match chrono::DateTime::parse_from_rfc3339(&relay_timestamp) {
        Ok(ts) => {
            let age = chrono::Utc::now().signed_duration_since(ts);
            if age > chrono::Duration::minutes(5) {
                tracing::warn!("[federation] Replay rejected: timestamp too old ({} ago)", age);
                return (
                    StatusCode::OK,
                    Json(protocol::new_error_value(id, -32026, "Relay timestamp too old (>5 min)")),
                );
            }
            if age < chrono::Duration::minutes(-1) {
                tracing::warn!("[federation] Replay rejected: timestamp in future");
                return (
                    StatusCode::OK,
                    Json(protocol::new_error_value(id, -32026, "Relay timestamp in future")),
                );
            }
        }
        Err(e) => {
            return (
                StatusCode::OK,
                Json(protocol::new_error_value(
                    id,
                    -32026,
                    &format!("Invalid relay_timestamp: {}", e),
                )),
            );
        }
    }

    // Check dedup cache: reject if relay_id already seen (D-04: persisted in redb)
    match state.persistence.check_and_insert_relay_id(&relay_id, &relay_timestamp) {
        Ok(true) => {
            tracing::info!("[federation] Replay check passed: relay_id={}", relay_id);
        }
        Ok(false) => {
            tracing::warn!("[federation] Replay rejected: duplicate relay_id={}", relay_id);
            return (
                StatusCode::OK,
                Json(protocol::new_error_value(
                    id,
                    -32026,
                    "Duplicate relay_id (replay detected)",
                )),
            );
        }
        Err(e) => {
            tracing::error!("[federation] Dedup cache error: {}", e);
            return (
                StatusCode::OK,
                Json(protocol::new_error_value(
                    id,
                    -32602,
                    &format!("Dedup check failed: {}", e),
                )),
            );
        }
    }

    // 3. Verify server signature (FAUT-03 layer 2)
    let remote_identity = match state.remote_cache.get(&from_server) {
        Some(id) => id,
        None => {
            tracing::warn!("Unknown server: {}", from_server);
            return (
                StatusCode::OK,
                Json(protocol::new_error_value(
                    id,
                    -32011,
                    "Unknown server identity",
                )),
            );
        }
    };

    let remote_pk_bytes = URL_SAFE_NO_PAD
        .decode(&remote_identity.public_key_b64)
        .unwrap_or_default();

    if !crate::signing::verify_relay_signature(&remote_pk_bytes, &params).unwrap_or(false) {
        tracing::error!(
            "Server signature verification failed for {}",
            from_server
        );
        return (
            StatusCode::OK,
            Json(protocol::new_error_value(
                id,
                -32010,
                "Server signature verification failed",
            )),
        );
    }
    tracing::info!("[federation] Server signature verified: {}", from_server);

    // 3. Verify individual message signatures (FAUT-03 layer 3)
    let messages = params
        .get("messages")
        .and_then(|m| m.as_array())
        .cloned()
        .unwrap_or_default();

    for msg in &messages {
        let sender = msg.get("from").and_then(|f| f.as_str()).unwrap_or("");
        if let Some(pk_bytes) = state.identity_store.get_public_key_bytes(sender) {
            if !crate::signing::verify_signature(&pk_bytes, msg).unwrap_or(false) {
                tracing::error!("Message signature failed for sender: {}", sender);
                return (
                    StatusCode::OK,
                    Json(protocol::new_error_value(
                        id,
                        -32010,
                        "Message signature verification failed",
                    )),
                );
            }
            tracing::info!("[federation] Message signature verified: {}", sender);
        } else {
            tracing::warn!("Unknown sender identity: {}", sender);
            // For Phase 7 demo, reject unknown senders
            return (
                StatusCode::OK,
                Json(protocol::new_error_value(
                    id,
                    -32011,
                    "Unknown sender identity",
                )),
            );
        }
    }

    // 4. Process messages: register thread, deliver locally, fan out if origin
    for msg in &messages {
        let thread_id = msg
            .get("thread_id")
            .and_then(|t| t.as_str())
            .unwrap_or("");
        let origin = parse_origin_domain(thread_id).unwrap_or("");

        // Register the thread if we don't know about it
        state
            .thread_registry
            .register_remote_thread(thread_id, origin);
        state
            .thread_registry
            .add_server(thread_id, &from_server);

        // Deliver to local participants via broadcast channel
        let deliver = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "ainm.message.deliver",
            "params": {
                "message": msg,
                "verified": true,
                "relayed_from": from_server,
            }
        });
        let _ = state
            .local_delivery_tx
            .send(serde_json::to_string(&deliver).unwrap());

        // If we are the origin server, assign sequence and fan out to other servers
        // Per RELY-03/D-11: origin server assigns sequence numbers and fans out
        if origin == state.server_id {
            if let Some(seq) = state.thread_registry.next_sequence(thread_id) {
                tracing::info!(
                    "[federation] Origin assigned seq {} for thread {}",
                    seq,
                    thread_id
                );
                // Store message in persistent thread history for backfill (BKFL-01)
                if let Err(e) = state
                    .persistence
                    .store_thread_message(thread_id, seq, msg)
                {
                    tracing::error!(
                        "[federation] Failed to store message for backfill: {}",
                        e
                    );
                }
                // Fan out to other participating servers (skip sender)
                if let Some(sender) = &state.relay_sender {
                    if let Some((_, servers)) = state.thread_registry.get_info(thread_id) {
                        for server in servers {
                            if server != from_server {
                                sender.enqueue(&server, msg.clone());
                            }
                        }
                    }
                }
            }
        }
    }

    // 5. Return success
    let result = serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {
            "accepted": true,
            "messages_processed": messages.len()
        }
    });
    (StatusCode::OK, Json(result))
}

// ---------------------------------------------------------------------------
// Backfill Handler
// ---------------------------------------------------------------------------

/// Handle ainm.backfill requests from remote servers.
///
/// Per D-16: Uses same relay endpoint, same mTLS auth.
/// Per D-17: Cursor-based pagination with after_sequence and limit.
/// Per D-18: Authorization via ThreadRegistry participating_servers check.
/// Per D-19: Post-join visibility only.
async fn handle_backfill(
    state: FederationState,
    id: serde_json::Value,
    body: serde_json::Value,
) -> (StatusCode, Json<serde_json::Value>) {
    let params = match body.get("params") {
        Some(p) => p,
        None => {
            return (
                StatusCode::OK,
                Json(protocol::new_error_value(id, -32602, "Missing params")),
            );
        }
    };

    // Parse backfill params
    let thread_id = match params.get("thread_id").and_then(|t| t.as_str()) {
        Some(t) => t.to_string(),
        None => {
            return (
                StatusCode::OK,
                Json(protocol::new_error_value(
                    id,
                    -32602,
                    "Missing thread_id",
                )),
            );
        }
    };

    let after_sequence = params
        .get("after_sequence")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);

    let limit = params
        .get("limit")
        .and_then(|v| v.as_u64())
        .unwrap_or(50)
        .min(200) as usize;

    // D-18: Authorization -- requesting server must be a participant
    let from_server = match params.get("from_server").and_then(|s| s.as_str()) {
        Some(s) => s.to_string(),
        None => {
            return (
                StatusCode::OK,
                Json(protocol::new_error_value(
                    id,
                    -32602,
                    "Missing from_server",
                )),
            );
        }
    };

    if !state
        .thread_registry
        .is_participant(&thread_id, &from_server)
    {
        tracing::warn!(
            "[backfill] Rejected: {} is not a participant in {}",
            from_server,
            thread_id
        );
        return (
            StatusCode::OK,
            Json(protocol::new_error_value(
                id,
                -32025,
                "Not a participant in this thread",
            )),
        );
    }

    // D-19: Post-join visibility -- only return messages after the server's join sequence
    let join_seq = state
        .thread_registry
        .get_join_sequence(&thread_id, &from_server)
        .unwrap_or(0);
    let effective_after = after_sequence.max(join_seq);

    // Check this is the origin server (only origin has the full message history)
    let origin = parse_origin_domain(&thread_id).unwrap_or("");
    if origin != state.server_id {
        return (
            StatusCode::OK,
            Json(protocol::new_error_value(
                id,
                -32024,
                "Backfill only available from origin server",
            )),
        );
    }

    // Retrieve messages from persistence
    match state
        .persistence
        .get_thread_messages(&thread_id, effective_after, limit)
    {
        Ok((messages, has_more, next_cursor)) => {
            tracing::info!(
                "[backfill] Returning {} messages for {} (after_seq={}, join_seq={}, has_more={})",
                messages.len(),
                thread_id,
                after_sequence,
                join_seq,
                has_more
            );
            let mut result = serde_json::json!({
                "messages": messages,
                "has_more": has_more,
            });
            if let Some(cursor) = next_cursor {
                result
                    .as_object_mut()
                    .unwrap()
                    .insert("next_cursor".to_string(), serde_json::json!(cursor));
            }
            (
                StatusCode::OK,
                Json(serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": result,
                })),
            )
        }
        Err(e) => {
            tracing::error!("[backfill] Failed to retrieve messages: {}", e);
            (
                StatusCode::OK,
                Json(protocol::new_error_value(
                    id,
                    -32603,
                    &format!("Backfill failed: {}", e),
                )),
            )
        }
    }
}

/// Request backfill from origin server for a thread.
/// Used for testing and as a reference client implementation.
pub async fn request_backfill(
    http_client: &reqwest::Client,
    origin_relay_endpoint: &str,
    from_server: &str,
    thread_id: &str,
    after_sequence: u64,
    limit: u64,
) -> Result<serde_json::Value, String> {
    let payload = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "ainm.backfill",
        "params": {
            "thread_id": thread_id,
            "from_server": from_server,
            "after_sequence": after_sequence,
            "limit": limit,
        }
    });
    let resp = http_client
        .post(origin_relay_endpoint)
        .json(&payload)
        .send()
        .await
        .map_err(|e| format!("Backfill request failed: {}", e))?;
    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("Backfill response parse failed: {}", e))?;
    Ok(body)
}
