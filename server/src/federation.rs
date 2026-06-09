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
#[allow(dead_code)]
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
    /// HTTP client for lazy identity resolution (mTLS-configured).
    pub http_client: Option<reqwest::Client>,
    /// Shared with the WS listener's ServerState (P3a) so relayed messages reach connected agents
    /// and apply the same consent + hop guard. Same Arc-backed instances as the WS listener.
    pub agent_registry: crate::server::AgentRegistry,
    pub human_registry: crate::server::HumanRegistry,
    pub a2a_hops: std::sync::Arc<tokio::sync::Mutex<std::collections::HashMap<String, u32>>>,
    pub a2a_hop_cap: u32,
    pub hosted_agent: Option<std::sync::Arc<crate::hosted_agent::HostedAgent>>,
    /// P3b: the WS listener's `ServerState`, shared so the relay path can spawn a hosted
    /// assistant turn for a relayed message addressed to this server's hosted agent.
    /// `None` in tests / when no hosted agent is provisioned.
    pub hosted_turn_state: Option<std::sync::Arc<crate::server::ServerState>>,
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
    let method = body.get("method").and_then(|m| m.as_str()).unwrap_or("");
    let id = body.get("id").cloned().unwrap_or(serde_json::json!(1));

    match method {
        "ainm.relay" => {} // fall through to relay handling below
        "ainm.backfill" => {
            return handle_backfill(state, id, body).await;
        }
        "ainm.context-push" => {
            return handle_context_push(state, id, body).await;
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
                Json(protocol::new_error_value(id, -32602, "Missing from_server")),
            )
        }
    };

    // 2. Replay protection (FAUT-05: D-01, D-03, D-04)
    let relay_timestamp = match params.get("relay_timestamp").and_then(|t| t.as_str()) {
        Some(t) => t.to_string(),
        None => {
            return (
                StatusCode::OK,
                Json(protocol::new_error_value(
                    id,
                    -32026,
                    "Missing relay_timestamp",
                )),
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
                tracing::warn!(
                    "[federation] Replay rejected: timestamp too old ({} ago)",
                    age
                );
                return (
                    StatusCode::OK,
                    Json(protocol::new_error_value(
                        id,
                        -32026,
                        "Relay timestamp too old (>5 min)",
                    )),
                );
            }
            if age < chrono::Duration::minutes(-1) {
                tracing::warn!("[federation] Replay rejected: timestamp in future");
                return (
                    StatusCode::OK,
                    Json(protocol::new_error_value(
                        id,
                        -32026,
                        "Relay timestamp in future",
                    )),
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
    match state
        .persistence
        .check_and_insert_relay_id(&relay_id, &relay_timestamp)
    {
        Ok(true) => {
            tracing::info!("[federation] Replay check passed: relay_id={}", relay_id);
        }
        Ok(false) => {
            tracing::warn!(
                "[federation] Replay rejected: duplicate relay_id={}",
                relay_id
            );
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
    // Lazy identity resolution: if server is unknown, try to resolve it now.
    // This handles the startup race where Server B starts before Server A.
    if state.remote_cache.get(&from_server).is_none() {
        if let Some(ref client) = state.http_client {
            // Try to find the remote server's federation port from the from_server domain
            // Use the relay endpoint pattern: from_server is the server_id (domain)
            // We need to find their federation port. Check if we have a --remote configured
            // that matches this domain.
            tracing::info!(
                "[federation] Lazily resolving unknown server: {}",
                from_server
            );
            // Try common federation ports based on the from_server identity
            // The relay_endpoint in their identity response will tell us the correct port
            for port in &[8443u16, 9443] {
                let url = format!("https://{}:{}/.well-known/ainm/server", from_server, port);
                if let Ok(resp) = client.get(&url).send().await {
                    if let Ok(body) = resp.json::<serde_json::Value>().await {
                        let resolved = crate::identity::ServerIdentity {
                            server_id: body
                                .get("server_id")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string(),
                            public_key_b64: body
                                .get("public_key")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string(),
                            relay_endpoint: body
                                .get("relay_endpoint")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string(),
                            tls_fingerprints: body
                                .get("tls_fingerprints")
                                .and_then(|v| v.as_array())
                                .map(|arr| {
                                    arr.iter()
                                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                                        .collect()
                                })
                                .unwrap_or_default(),
                            supported_versions: vec!["0.1.0".to_string()],
                        };
                        if !resolved.server_id.is_empty() {
                            tracing::info!("[federation] Lazily resolved: {}", resolved.server_id);
                            state.remote_cache.insert(resolved);
                            break;
                        }
                    }
                }
            }
        }
    }

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
        tracing::error!("Server signature verification failed for {}", from_server);
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
            // Lazily fetch sender's public key from origin server
            let mut resolved = false;
            if let Some(ref client) = state.http_client {
                // Extract domain from sender address (user@domain)
                if let Some(domain) = sender.split('@').nth(1) {
                    if let Some(remote_id) = state.remote_cache.get(domain) {
                        // Build participant key URL from relay_endpoint base
                        let base = remote_id.relay_endpoint.replace("/ainm/v1/relay", "");
                        let url = format!("{}/.well-known/ainm/participant/{}", base, sender);
                        if let Ok(resp) = client.get(&url).send().await {
                            if let Ok(body) = resp.json::<serde_json::Value>().await {
                                if let Some(pk) = body.get("public_key").and_then(|v| v.as_str()) {
                                    if let Ok(pk_bytes) =
                                        base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(pk)
                                    {
                                        let type_ = body
                                            .get("type")
                                            .and_then(|v| v.as_str())
                                            .unwrap_or("human");
                                        state
                                            .identity_store
                                            .register(sender, pk, type_, domain, None);
                                        tracing::info!(
                                            "[federation] Lazily fetched key for {}",
                                            sender
                                        );
                                        // Now verify the message signature
                                        if crate::signing::verify_signature(&pk_bytes, msg)
                                            .unwrap_or(false)
                                        {
                                            tracing::info!(
                                                "[federation] Message signature verified: {}",
                                                sender
                                            );
                                            resolved = true;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            if !resolved {
                tracing::warn!("Unknown sender identity: {}", sender);
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
    }

    // 4. Process messages: register thread, deliver locally, fan out if origin
    for msg in &messages {
        let thread_id = msg.get("thread_id").and_then(|t| t.as_str()).unwrap_or("");
        let origin = parse_origin_domain(thread_id).unwrap_or("");

        // Register the thread if we don't know about it
        state
            .thread_registry
            .register_remote_thread(thread_id, origin);
        state.thread_registry.add_server(thread_id, &from_server);

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

        // Runtime A2A P3a: also route through the shared delivery orchestrator so consent + the
        // hop guard apply on THIS (receiving) server and a connected connector agent actually
        // receives the message. sender_type is derived from the registered identity (resolved +
        // verified in step 3 above); default "human". P3b: the relay path now carries the WS
        // listener's ServerState (`hosted_turn_state`), so a relayed message addressed to this
        // server's hosted assistant spawns its turn (state_for_hosted_turn passed below).
        {
            let sender = msg.get("from").and_then(|f| f.as_str()).unwrap_or("");
            let sender_type = match state.identity_store.participant_type(sender).as_deref() {
                Some("agent") => "agent",
                _ => "human",
            };

            // Runtime A2A P3b: a signature-verified relayed AGENT sender is auto-invited into the
            // thread on THIS (receiving) server. The relay arrived over mTLS and the per-message
            // Ed25519 signature was verified above (step 3), so the origin server vouches for the
            // sender as a real thread participant. This satisfies the P2 consent gate's
            // "sender is human-invited" half for cross-server peers without a redundant manual
            // invite on the receiving side. (The TARGET's invitation remains B's own
            // human-curated membership — established when a human's relayed message invited it.)
            if sender_type == "agent" && !sender.is_empty() && !thread_id.is_empty() {
                state
                    .agent_registry
                    .invite_to_thread(sender, thread_id)
                    .await;
            }

            let deps = crate::server::DeliverDeps {
                human_registry: &state.human_registry,
                agent_registry: &state.agent_registry,
                a2a_hops: &state.a2a_hops,
                a2a_hop_cap: state.a2a_hop_cap,
                hosted_agent: state.hosted_agent.as_deref(),
                to_agent_tx: &state.local_delivery_tx,
                // Federation relay has no DB handle (cross-server is v2); block
                // filtering applies on the local single-server delivery path.
                db: None,
                state_for_hosted_turn: state.hosted_turn_state.as_deref(),
            };
            crate::server::deliver_message(&deps, msg, sender_type, true).await;
        }

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
                if let Err(e) = state.persistence.store_thread_message(thread_id, seq, msg) {
                    tracing::error!("[federation] Failed to store message for backfill: {}", e);
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
                Json(protocol::new_error_value(id, -32602, "Missing thread_id")),
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
                Json(protocol::new_error_value(id, -32602, "Missing from_server")),
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

// ---------------------------------------------------------------------------
// Context Push Handler
// ---------------------------------------------------------------------------

/// Handle ainm.context-push requests from remote servers.
///
/// D-09: Server-level signature verification only (Layer 2).
/// No per-message signatures needed -- this is a server-to-server operation.
/// Layer 2 verification is performed before the method dispatch match in relay_handler
/// for ainm.relay, but context-push arrives at the same endpoint. In this implementation,
/// the mTLS + FAUT-02 fingerprint check at the TLS acceptor level provides Layer 1,
/// and we trust the authenticated server identity.
///
/// Delivers received context frames to local participants via broadcast channel
/// using the ainm.context.frame method so REPL/agents can render them.
async fn handle_context_push(
    state: FederationState,
    id: serde_json::Value,
    body: serde_json::Value,
) -> (StatusCode, Json<serde_json::Value>) {
    let params = match body.get("params") {
        Some(p) => p.clone(),
        None => {
            return (
                StatusCode::OK,
                Json(protocol::new_error_value(id, -32602, "Missing params")),
            );
        }
    };

    let thread_id = params
        .get("thread_id")
        .and_then(|t| t.as_str())
        .unwrap_or("");
    let frames = params.get("context_frames").and_then(|f| f.as_array());

    if thread_id.is_empty() || frames.is_none() {
        return (
            StatusCode::OK,
            Json(protocol::new_error_value(
                id,
                -32602,
                "Missing thread_id or context_frames",
            )),
        );
    }

    tracing::info!(
        "[federation] Received context-push for thread {} with {} frames",
        thread_id,
        frames.unwrap().len()
    );

    // Deliver context frames to local participants via broadcast channel
    // Use ainm.context.frame method so REPL/agents can render them
    let deliver = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "ainm.context.frame",
        "params": params,
    });
    let _ = state
        .local_delivery_tx
        .send(serde_json::to_string(&deliver).unwrap());

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": { "accepted": true }
        })),
    )
}

#[allow(dead_code)]
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

// ---------------------------------------------------------------------------
// In-process federation relay tests (no Postgres, no network)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use axum::extract::State;
    use axum::Json;
    use std::time::Duration;

    /// Build a redb-backed PersistenceLayer in a temp directory.
    fn make_persistence() -> crate::persistence::PersistenceLayer {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir
            .path()
            .join(format!("fed-test-{}.redb", uuid::Uuid::new_v4()))
            .to_string_lossy()
            .into_owned();
        // keep `dir` alive by leaking it for the duration of the test process
        std::mem::forget(dir);
        crate::persistence::PersistenceLayer::open(&path).expect("PersistenceLayer::open")
    }

    /// Build a signed inner message envelope as sent by `alpha` to `beta`.
    fn build_inner_msg(
        alpha: &crate::identity::Identity,
        thread_id: &str,
        beta_addr: &str,
    ) -> serde_json::Value {
        let mut msg = serde_json::json!({
            "id": uuid::Uuid::new_v4().to_string(),
            "from": alpha.address,
            "to": beta_addr,
            "thread_id": thread_id,
            "timestamp": chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            "type": "message",
            "content": [{
                "type": "text",
                "version": "1.0",
                "source": "agent",
                "text": "hello from alpha",
                "fallback": "hello from alpha"
            }]
        });
        crate::signing::sign_envelope(&alpha.key_pair, &mut msg).expect("sign_envelope");
        msg
    }

    /// Build the full `ainm.relay` JSON-RPC payload, server-signed by server A.
    fn build_relay_payload(
        server_a: &crate::identity::Identity,
        messages: Vec<serde_json::Value>,
    ) -> serde_json::Value {
        let mut params = serde_json::json!({
            "messages": messages,
            "from_server": "server-a.local",
            "relay_timestamp": chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            "relay_id": uuid::Uuid::new_v4().to_string(),
        });
        crate::signing::sign_relay_params(&server_a.key_pair, &mut params)
            .expect("sign_relay_params");
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "ainm.relay",
            "params": params
        })
    }

    /// Task 4 (A2A-P3a): relay_handler → deliver_message → connected agent.
    ///
    /// Proves that a valid `ainm.relay` payload arriving at `relay_handler` results
    /// in delivery to a connected agent registered in the shared `AgentRegistry`.
    /// No Postgres, no network: entirely in-process using redb (temp file).
    #[tokio::test]
    async fn relay_delivers_to_connected_agent() {
        // 1. Persistence (redb, temp file)
        let persistence = make_persistence();

        // 2. Server B identity (the receiving server under test)
        let server_b_kp =
            crate::identity::generate_keypair("server-b.local").expect("server-b keypair");
        let server_b_identity =
            crate::identity::build_server_identity("server-b.local", &server_b_kp, 8443, vec![]);

        // 3. Server A identity (the relaying peer)
        let server_a_kp =
            crate::identity::generate_keypair("server-a.local").expect("server-a keypair");
        let remote_cache = crate::identity::RemoteServerCache::new();
        remote_cache.insert(crate::identity::ServerIdentity {
            server_id: "server-a.local".to_string(),
            public_key_b64: server_a_kp.public_key_b64(),
            relay_endpoint: "https://server-a.local:8443/ainm/v1/relay".to_string(),
            tls_fingerprints: vec![],
            supported_versions: vec!["0.1.0".to_string()],
        });

        // 4. Identity store on B: register alpha (sender from A)
        let alpha_kp =
            crate::identity::generate_keypair("alpha@server-a.local").expect("alpha keypair");
        let identity_store = crate::identity::IdentityStore::new();
        identity_store.register(
            "alpha@server-a.local",
            &alpha_kp.public_key_b64(),
            "agent",
            "server-a.local",
            None,
        );

        // 5. Beta: connected agent on B — register in AgentRegistry, keep receiver
        let agent_registry = crate::server::AgentRegistry::new();
        let beta_card = serde_json::json!({
            "name": "beta",
            "ainm": { "acceptsA2A": true }
        });
        let mut beta_rx = agent_registry
            .register("beta@server-b.local", &beta_card)
            .await;

        // 6. Thread ID with origin server-a.local (so B is NOT origin → relay_sender not needed)
        let thread_id = format!("server-a.local:{}", uuid::Uuid::now_v7());

        // 7. P2 consent: a human on B invited the TARGET (beta). The foreign SENDER (alpha) is
        //    deliberately NOT invited here — P3b must auto-invite the verified relayed agent
        //    sender, or consent_ok would deny this agent→agent message.
        agent_registry
            .invite_to_thread("beta@server-b.local", &thread_id)
            .await;

        // 8. Build FederationState
        let (local_delivery_tx, _local_delivery_rx) = tokio::sync::broadcast::channel::<String>(64);
        let a2a_hops = std::sync::Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::<
            String,
            u32,
        >::new()));
        let fed_state = FederationState {
            server_id: "server-b.local".to_string(),
            server_keypair: std::sync::Arc::new(server_b_kp),
            server_identity: std::sync::Arc::new(server_b_identity),
            thread_registry: ThreadRegistry::new(),
            identity_store,
            remote_cache,
            relay_sender: None,
            local_delivery_tx,
            persistence,
            http_client: None,
            agent_registry: agent_registry.clone(),
            human_registry: crate::server::HumanRegistry::new(),
            a2a_hops,
            a2a_hop_cap: 6,
            hosted_agent: None,
            hosted_turn_state: None,
        };

        // 9. Build signed inner message (alpha → beta)
        let inner_msg = build_inner_msg(&alpha_kp, &thread_id, "beta@server-b.local");

        // 10. Build signed relay payload (signed by server A)
        let payload = build_relay_payload(&server_a_kp, vec![inner_msg]);

        // 11. Call relay_handler
        let (status, body) = relay_handler(State(fed_state), Json(payload)).await;
        let body_val = body.0;

        assert_eq!(
            status,
            axum::http::StatusCode::OK,
            "relay_handler returned non-200: {:?}",
            body_val
        );
        // Ensure no error code in the response
        assert!(
            body_val.get("error").is_none(),
            "relay_handler returned an error: {:?}",
            body_val
        );
        assert_eq!(
            body_val
                .get("result")
                .and_then(|r| r.get("accepted"))
                .and_then(|a| a.as_bool()),
            Some(true),
            "Expected result.accepted == true, got: {:?}",
            body_val
        );

        // 12. Assert beta's receiver got an ainm.message.deliver
        let got = tokio::time::timeout(Duration::from_secs(2), beta_rx.recv())
            .await
            .expect("timeout waiting for beta to receive message")
            .expect("beta receiver closed before message arrived");

        assert!(
            got.contains("ainm.message.deliver"),
            "Expected ainm.message.deliver in beta's message, got: {}",
            got
        );
        assert!(
            got.contains("beta@server-b.local"),
            "Expected beta@server-b.local in message, got: {}",
            got
        );
        assert!(
            got.contains("hello from alpha"),
            "Expected message content in delivery, got: {}",
            got
        );
    }

    /// P3b: when the TARGET agent was never human-invited on B, the auto-invited foreign
    /// sender is not enough — consent_ok still denies (both endpoints must be invited).
    /// Asserts beta receives NOTHING (the message drops to humans-only with a notice).
    #[tokio::test]
    async fn relay_denied_when_target_not_invited() {
        let persistence = make_persistence();

        let server_b_kp =
            crate::identity::generate_keypair("server-b.local").expect("server-b keypair");
        let server_b_identity =
            crate::identity::build_server_identity("server-b.local", &server_b_kp, 8443, vec![]);

        let server_a_kp =
            crate::identity::generate_keypair("server-a.local").expect("server-a keypair");
        let remote_cache = crate::identity::RemoteServerCache::new();
        remote_cache.insert(crate::identity::ServerIdentity {
            server_id: "server-a.local".to_string(),
            public_key_b64: server_a_kp.public_key_b64(),
            relay_endpoint: "https://server-a.local:8443/ainm/v1/relay".to_string(),
            tls_fingerprints: vec![],
            supported_versions: vec!["0.1.0".to_string()],
        });

        let alpha_kp =
            crate::identity::generate_keypair("alpha@server-a.local").expect("alpha keypair");
        let identity_store = crate::identity::IdentityStore::new();
        identity_store.register(
            "alpha@server-a.local",
            &alpha_kp.public_key_b64(),
            "agent",
            "server-a.local",
            None,
        );

        let agent_registry = crate::server::AgentRegistry::new();
        let beta_card = serde_json::json!({ "name": "beta", "ainm": { "acceptsA2A": true } });
        let mut beta_rx = agent_registry
            .register("beta@server-b.local", &beta_card)
            .await;

        let thread_id = format!("server-a.local:{}", uuid::Uuid::now_v7());

        // NOTE: beta is NOT invited here. alpha will be auto-invited by relay_handler, but
        // consent requires BOTH — so delivery to beta must be denied.

        let (local_delivery_tx, _rx) = tokio::sync::broadcast::channel::<String>(64);
        let a2a_hops = std::sync::Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::<
            String,
            u32,
        >::new()));
        let fed_state = FederationState {
            server_id: "server-b.local".to_string(),
            server_keypair: std::sync::Arc::new(server_b_kp),
            server_identity: std::sync::Arc::new(server_b_identity),
            thread_registry: ThreadRegistry::new(),
            identity_store,
            remote_cache,
            relay_sender: None,
            local_delivery_tx,
            persistence,
            http_client: None,
            agent_registry: agent_registry.clone(),
            human_registry: crate::server::HumanRegistry::new(),
            a2a_hops,
            a2a_hop_cap: 6,
            hosted_agent: None,
            hosted_turn_state: None,
        };

        let inner_msg = build_inner_msg(&alpha_kp, &thread_id, "beta@server-b.local");
        let payload = build_relay_payload(&server_a_kp, vec![inner_msg]);

        let (status, body) = relay_handler(State(fed_state), Json(payload)).await;
        assert_eq!(status, axum::http::StatusCode::OK);
        assert!(
            body.0.get("error").is_none(),
            "relay returned error: {:?}",
            body.0
        );

        // beta must NOT receive the message (consent denied — target not invited).
        let res = tokio::time::timeout(Duration::from_millis(300), beta_rx.recv()).await;
        assert!(
            res.is_err(),
            "expected NO delivery to beta (consent denied), but got: {:?}",
            res
        );
    }
}
