//! Outbound federation relay sender with redb-backed persistent queues and
//! exponential backoff.
//!
//! Implements transport.md Section 5.3:
//! - Constructs signed `ainm.relay` JSON-RPC 2.0 payloads
//! - Per-destination persistent message queues (FREL-01: survives restart)
//! - Exponential backoff per destination (RELY-05: 1s base, 60s max)
//! - 10K per-destination bound with FIFO eviction (D-11/D-13)

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

// ---------------------------------------------------------------------------
// Backoff State (in-memory -- resets on restart per D-14)
// ---------------------------------------------------------------------------

/// Tracks exponential backoff state for a single destination.
struct BackoffState {
    attempt: u32,
    base_delay: Duration,
    max_delay: Duration,
    next_retry: Option<Instant>,
}

impl BackoffState {
    fn new() -> Self {
        Self {
            attempt: 0,
            base_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(60),
            next_retry: None,
        }
    }

    fn record_failure(&mut self) {
        self.attempt += 1;
        let delay = self.base_delay * 2u32.pow(self.attempt.min(6));
        self.next_retry = Some(Instant::now() + delay.min(self.max_delay));
    }

    fn record_success(&mut self) {
        self.attempt = 0;
        self.next_retry = None;
    }

    fn can_retry(&self) -> bool {
        self.next_retry.is_none_or(|t| Instant::now() >= t)
    }
}

// ---------------------------------------------------------------------------
// Relay Sender (redb-backed persistent queue)
// ---------------------------------------------------------------------------

/// Outbound relay sender with redb-backed persistent message queues.
///
/// Messages are persisted immediately via `PersistenceLayer` (FREL-01: survives
/// server restart) and flushed by a background loop. Failed deliveries trigger
/// exponential backoff (RELY-05: 1s base, 60s max, 2^n growth capped at 2^6).
/// Backoff state is in-memory and resets on restart (D-14).
pub struct RelaySender {
    backoff_states: Arc<Mutex<HashMap<String, BackoffState>>>,
    persistence: crate::persistence::PersistenceLayer,
    http_client: reqwest::Client,
    server_keypair: Arc<crate::identity::Identity>,
    server_id: String,
    remote_cache: crate::identity::RemoteServerCache,
}

impl RelaySender {
    /// Create a new relay sender with redb-backed persistent queue.
    ///
    /// * `http_client` - reqwest client configured for mTLS (from `tls::build_mtls_client`)
    /// * `server_keypair` - this server's Ed25519 identity for signing relay payloads
    /// * `server_id` - this server's domain (e.g., "server-a.local")
    /// * `remote_cache` - cache of resolved remote server identities
    /// * `persistence` - redb persistence layer for offline queue storage
    pub fn new(
        http_client: reqwest::Client,
        server_keypair: Arc<crate::identity::Identity>,
        server_id: String,
        remote_cache: crate::identity::RemoteServerCache,
        persistence: crate::persistence::PersistenceLayer,
    ) -> Self {
        Self {
            backoff_states: Arc::new(Mutex::new(HashMap::new())),
            persistence,
            http_client,
            server_keypair,
            server_id,
            remote_cache,
        }
    }

    /// Enqueue a message for delivery to a remote server.
    ///
    /// Persists to redb (FREL-01: survives restart). Enforces 10K per-destination
    /// bound with FIFO eviction of oldest messages (D-11/D-13).
    pub fn enqueue(&self, destination: &str, message: serde_json::Value) {
        // Persist to redb (FREL-01: survives restart)
        if let Err(e) = self.persistence.enqueue_offline(destination, &message) {
            tracing::error!(
                "[relay] Failed to persist message for {}: {}",
                destination,
                e
            );
            return;
        }

        // Enforce 10K bound per destination (D-11: FIFO eviction)
        match self.persistence.queue_len(destination) {
            Ok(len) if len > 10_000 => match self.persistence.evict_oldest(destination, 10_000) {
                Ok(evicted) => {
                    tracing::warn!(
                        "[relay] Queue bound exceeded for {}: evicted {} oldest messages",
                        destination,
                        evicted
                    );
                }
                Err(e) => tracing::error!("[relay] Eviction failed for {}: {}", destination, e),
            },
            _ => {}
        }

        tracing::info!("[relay] Enqueued message for {} (persistent)", destination);
    }

    /// Send queued messages for a destination. Called by the send loop.
    /// Drains up to 100 messages per flush (batch processing).
    pub async fn flush_destination(&self, destination: &str) -> Result<(), String> {
        // Check backoff
        {
            let backoffs = self.backoff_states.lock().unwrap();
            if let Some(backoff) = backoffs.get(destination) {
                if !backoff.can_retry() {
                    return Ok(()); // Still in backoff window
                }
            }
        }

        // Drain messages from persistent queue (batch of up to 100)
        let messages = self.persistence.drain_offline(destination, 100)?;
        if messages.is_empty() {
            return Ok(());
        }

        // Look up remote server's relay endpoint
        let remote = self
            .remote_cache
            .get(destination)
            .ok_or_else(|| format!("Unknown destination: {}", destination))?;

        // Build relay payload per transport.md Section 5.3
        // Include replay protection fields (FAUT-05: D-01/D-02/D-03)
        let mut params = serde_json::json!({
            "messages": messages,
            "from_server": self.server_id,
            "relay_timestamp": chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            "relay_id": uuid::Uuid::now_v7().to_string(),
        });

        // Sign relay params with server key (FAUT-03: server-level signature)
        crate::signing::sign_relay_params(&self.server_keypair.key_pair, &mut params)
            .map_err(|e| format!("Signing failed: {}", e))?;

        let payload = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "ainm.relay",
            "params": params,
        });

        let msg_count = messages.len();
        tracing::info!(
            "[relay] Sending {} message(s) to {}",
            msg_count,
            remote.relay_endpoint
        );

        // POST to relay endpoint
        match self
            .http_client
            .post(&remote.relay_endpoint)
            .json(&payload)
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => {
                // Success: reset backoff (D-14: reset on reconnection)
                let mut backoffs = self.backoff_states.lock().unwrap();
                backoffs
                    .entry(destination.to_string())
                    .or_insert_with(BackoffState::new)
                    .record_success();
                tracing::info!("[relay] Successfully relayed to {}", destination);
                Ok(())
            }
            Ok(resp) => {
                let status = resp.status();
                let body = resp.text().await.unwrap_or_default();
                tracing::error!(
                    "[relay] Relay to {} failed: {} - {}",
                    destination,
                    status,
                    body
                );
                // Re-enqueue messages back to persistence
                for msg in &messages {
                    let _ = self.persistence.enqueue_offline(destination, msg);
                }
                let mut backoffs = self.backoff_states.lock().unwrap();
                backoffs
                    .entry(destination.to_string())
                    .or_insert_with(BackoffState::new)
                    .record_failure();
                Err(format!("HTTP {}", status))
            }
            Err(e) => {
                tracing::error!("[relay] Connection to {} failed: {}", destination, e);
                // Re-enqueue messages back to persistence
                for msg in &messages {
                    let _ = self.persistence.enqueue_offline(destination, msg);
                }
                let mut backoffs = self.backoff_states.lock().unwrap();
                backoffs
                    .entry(destination.to_string())
                    .or_insert_with(BackoffState::new)
                    .record_failure();
                Err(format!("Connection error: {}", e))
            }
        }
    }

    /// Get list of destinations with pending messages (from persistent store).
    pub fn pending_destinations(&self) -> Vec<String> {
        self.persistence.pending_destinations().unwrap_or_default()
    }

    /// Start the background send loop. Checks all queues every 500ms.
    pub fn start_send_loop(self: &Arc<Self>) -> tokio::task::JoinHandle<()> {
        let sender = Arc::clone(self);
        tokio::spawn(async move {
            loop {
                let destinations = sender.pending_destinations();
                for dest in destinations {
                    let _ = sender.flush_destination(&dest).await;
                }
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
        })
    }
}
