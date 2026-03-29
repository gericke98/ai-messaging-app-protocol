//! Outbound federation relay sender with per-destination send queues and
//! exponential backoff.
//!
//! Implements transport.md Section 5.3:
//! - Constructs signed `ainm.relay` JSON-RPC 2.0 payloads
//! - Per-destination message queues (RELY-04: push-based, no polling)
//! - Exponential backoff per destination (RELY-05: 1s base, 60s max)

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

// ---------------------------------------------------------------------------
// Backoff State
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
        self.next_retry.map_or(true, |t| Instant::now() >= t)
    }
}

// ---------------------------------------------------------------------------
// Destination Queue
// ---------------------------------------------------------------------------

/// Per-destination message queue with backoff tracking.
struct DestinationQueue {
    messages: VecDeque<serde_json::Value>,
    backoff: BackoffState,
}

// ---------------------------------------------------------------------------
// Relay Sender
// ---------------------------------------------------------------------------

/// Outbound relay sender that manages per-destination send queues.
///
/// Messages are enqueued immediately (RELY-04: push-based) and flushed
/// by a background loop. Failed deliveries trigger exponential backoff
/// (RELY-05: 1s base, 60s max, 2^n growth capped at 2^6).
pub struct RelaySender {
    queues: Arc<Mutex<HashMap<String, DestinationQueue>>>,
    http_client: reqwest::Client,
    server_keypair: Arc<crate::identity::Identity>,
    server_id: String,
    remote_cache: crate::identity::RemoteServerCache,
}

impl RelaySender {
    /// Create a new relay sender.
    ///
    /// * `http_client` - reqwest client configured for mTLS (from `tls::build_mtls_client`)
    /// * `server_keypair` - this server's Ed25519 identity for signing relay payloads
    /// * `server_id` - this server's domain (e.g., "server-a.local")
    /// * `remote_cache` - cache of resolved remote server identities
    pub fn new(
        http_client: reqwest::Client,
        server_keypair: Arc<crate::identity::Identity>,
        server_id: String,
        remote_cache: crate::identity::RemoteServerCache,
    ) -> Self {
        Self {
            queues: Arc::new(Mutex::new(HashMap::new())),
            http_client,
            server_keypair,
            server_id,
            remote_cache,
        }
    }

    /// Enqueue a message for delivery to a remote server.
    ///
    /// Per RELY-04: push immediately, no polling.
    pub fn enqueue(&self, destination: &str, message: serde_json::Value) {
        let mut queues = self.queues.lock().unwrap();
        let queue = queues
            .entry(destination.to_string())
            .or_insert_with(|| DestinationQueue {
                messages: VecDeque::new(),
                backoff: BackoffState::new(),
            });
        queue.messages.push_back(message);
        tracing::info!("[relay] Enqueued message for {}", destination);
    }

    /// Send all queued messages for a destination. Called by the send loop.
    pub async fn flush_destination(&self, destination: &str) -> Result<(), String> {
        let messages: Vec<serde_json::Value> = {
            let mut queues = self.queues.lock().unwrap();
            if let Some(queue) = queues.get_mut(destination) {
                if !queue.backoff.can_retry() {
                    return Ok(()); // Still in backoff window
                }
                queue.messages.drain(..).collect()
            } else {
                return Ok(());
            }
        };

        if messages.is_empty() {
            return Ok(());
        }

        // Look up remote server's relay endpoint
        let remote = self
            .remote_cache
            .get(destination)
            .ok_or_else(|| format!("Unknown destination: {}", destination))?;

        // Build relay payload per transport.md Section 5.3
        let mut params = serde_json::json!({
            "messages": messages,
            "from_server": self.server_id,
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
                let mut queues = self.queues.lock().unwrap();
                if let Some(queue) = queues.get_mut(destination) {
                    queue.backoff.record_success();
                }
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
                // Re-enqueue messages and record failure
                let mut queues = self.queues.lock().unwrap();
                if let Some(queue) = queues.get_mut(destination) {
                    for msg in messages.into_iter().rev() {
                        queue.messages.push_front(msg);
                    }
                    queue.backoff.record_failure();
                }
                Err(format!("HTTP {}", status))
            }
            Err(e) => {
                tracing::error!("[relay] Connection to {} failed: {}", destination, e);
                let mut queues = self.queues.lock().unwrap();
                if let Some(queue) = queues.get_mut(destination) {
                    for msg in messages.into_iter().rev() {
                        queue.messages.push_front(msg);
                    }
                    queue.backoff.record_failure();
                }
                Err(format!("Connection error: {}", e))
            }
        }
    }

    /// Get list of destinations with pending messages.
    pub fn pending_destinations(&self) -> Vec<String> {
        self.queues
            .lock()
            .unwrap()
            .iter()
            .filter(|(_, q)| !q.messages.is_empty())
            .map(|(k, _)| k.clone())
            .collect()
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
