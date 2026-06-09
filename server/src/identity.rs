use anyhow::Result;
use axum::extract::State;
use axum::http::{header, StatusCode};
use axum::response::IntoResponse;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use ring::rand::SystemRandom;
use ring::signature::{Ed25519KeyPair, KeyPair};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Represents a participant's identity: address + Ed25519 key pair.
#[derive(Clone)]
pub struct Identity {
    pub address: String,
    pub key_pair: Arc<Ed25519KeyPair>,
    pub public_key_bytes: Vec<u8>,
}

impl Identity {
    /// Encode public key as base64url without padding.
    pub fn public_key_b64(&self) -> String {
        URL_SAFE_NO_PAD.encode(&self.public_key_bytes)
    }
}

/// Generate a new Ed25519 identity for a given AINM address.
pub fn generate_keypair(address: &str) -> Result<Identity> {
    let rng = SystemRandom::new();
    let pkcs8_doc = Ed25519KeyPair::generate_pkcs8(&rng)
        .map_err(|e| anyhow::anyhow!("key generation failed: {}", e))?;
    let key_pair = Ed25519KeyPair::from_pkcs8(pkcs8_doc.as_ref())
        .map_err(|e| anyhow::anyhow!("key parse failed: {}", e))?;
    let public_key_bytes = key_pair.public_key().as_ref().to_vec();
    Ok(Identity {
        address: address.to_string(),
        key_pair: Arc::new(key_pair),
        public_key_bytes,
    })
}

/// Identity record matching spec/identity.md Section 4.2.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentityRecord {
    pub id: String,
    pub public_key: String,
    pub server: String,
    #[serde(rename = "type")]
    pub type_: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_card_url: Option<String>,
    pub key_history: Vec<serde_json::Value>,
}

/// Identity store: in-memory registrations (legacy/federation pre-seeds) with an
/// optional durable fallback to the accounts store (`accounts` + `account_keys`).
/// The DB fallback is what makes identity resolution survive a server restart —
/// production never populates the in-memory map.
#[derive(Clone, Default)]
pub struct IdentityStore {
    inner: Arc<Mutex<HashMap<String, IdentityRecord>>>,
    db: Option<sqlx::PgPool>,
}

impl IdentityStore {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(HashMap::new())),
            db: None,
        }
    }

    /// Attach the durable accounts store as a resolution fallback.
    pub fn with_db(mut self, pool: sqlx::PgPool) -> Self {
        self.db = Some(pool);
        self
    }

    /// Resolve an address's public key: in-memory registrations first, then the
    /// durable accounts store (active account, active key).
    pub async fn resolve_public_key_bytes(&self, address: &str) -> Option<Vec<u8>> {
        if let Some(bytes) = self.get_public_key_bytes(address) {
            if !bytes.is_empty() {
                return Some(bytes);
            }
        }
        let pool = self.db.as_ref()?;
        let acct = crate::accounts::find_account_by_address(pool, address)
            .await
            .ok()??;
        let key = crate::accounts::find_active_key(pool, acct.id)
            .await
            .ok()??;
        URL_SAFE_NO_PAD.decode(&key.public_key).ok()
    }

    /// Register an identity with the store.
    pub fn register(
        &self,
        address: &str,
        public_key: &str,
        type_: &str,
        server: &str,
        agent_card_url: Option<String>,
    ) -> IdentityRecord {
        let record = IdentityRecord {
            id: address.to_string(),
            public_key: public_key.to_string(),
            server: server.to_string(),
            type_: type_.to_string(),
            agent_card_url,
            key_history: vec![],
        };
        self.inner
            .lock()
            .unwrap()
            .insert(address.to_string(), record.clone());
        record
    }

    /// Get the raw public key bytes for an address.
    pub fn get_public_key_bytes(&self, address: &str) -> Option<Vec<u8>> {
        self.inner
            .lock()
            .unwrap()
            .get(address)
            .map(|r| URL_SAFE_NO_PAD.decode(&r.public_key).unwrap_or_default())
    }

    /// The registered participant type ("human"/"agent") for an address, if known.
    pub fn participant_type(&self, address: &str) -> Option<String> {
        self.inner
            .lock()
            .unwrap()
            .get(address)
            .map(|r| r.type_.clone())
    }
}

// ---------------------------------------------------------------------------
// Server-level identity for federation
// ---------------------------------------------------------------------------

/// Server-level identity for federation (distinct from participant identity).
///
/// Published at `GET /.well-known/ainm/server` so remote servers can discover
/// this server's relay endpoint, public key, and TLS fingerprints.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerIdentity {
    pub server_id: String,
    pub public_key_b64: String,
    pub relay_endpoint: String,
    pub tls_fingerprints: Vec<String>,
    pub supported_versions: Vec<String>,
}

/// Build a `ServerIdentity` from server configuration values.
pub fn build_server_identity(
    server_id: &str,
    server_keypair: &Identity,
    federation_port: u16,
    tls_fingerprints: Vec<String>,
) -> ServerIdentity {
    ServerIdentity {
        server_id: server_id.to_string(),
        public_key_b64: server_keypair.public_key_b64(),
        relay_endpoint: format!("https://{}:{}/ainm/v1/relay", server_id, federation_port),
        tls_fingerprints,
        supported_versions: vec!["0.1.0".to_string()],
    }
}

/// Cached agent card with TTL-based expiry (D-03: 5-minute TTL).
#[allow(dead_code)]
pub struct AgentCardEntry {
    pub card: serde_json::Value,
    pub fetched_at: std::time::Instant,
    pub ttl: std::time::Duration,
}

impl AgentCardEntry {
    #[allow(dead_code)]
    pub fn is_expired(&self) -> bool {
        self.fetched_at.elapsed() > self.ttl
    }
}

/// In-memory cache of resolved remote server identities.
#[derive(Clone, Default)]
#[allow(dead_code)]
pub struct RemoteServerCache {
    inner: Arc<Mutex<HashMap<String, ServerIdentity>>>,
    agent_cards: Arc<Mutex<HashMap<String, AgentCardEntry>>>,
}

impl RemoteServerCache {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(HashMap::new())),
            agent_cards: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    #[allow(dead_code)]
    pub fn get_agent_card(&self, key: &str) -> Option<serde_json::Value> {
        let mut cards = self.agent_cards.lock().unwrap();
        if let Some(entry) = cards.get(key) {
            if entry.is_expired() {
                cards.remove(key);
                None
            } else {
                Some(entry.card.clone())
            }
        } else {
            None
        }
    }

    #[allow(dead_code)]
    pub fn insert_agent_card(&self, key: &str, card: serde_json::Value) {
        self.agent_cards.lock().unwrap().insert(
            key.to_string(),
            AgentCardEntry {
                card,
                fetched_at: std::time::Instant::now(),
                ttl: std::time::Duration::from_secs(300),
            },
        );
    }

    /// Look up a cached remote server identity by server_id.
    pub fn get(&self, server_id: &str) -> Option<ServerIdentity> {
        self.inner.lock().unwrap().get(server_id).cloned()
    }

    /// Cache a remote server identity.
    pub fn insert(&self, identity: ServerIdentity) {
        self.inner
            .lock()
            .unwrap()
            .insert(identity.server_id.clone(), identity);
    }

    /// Find a cached server identity by TLS certificate fingerprint.
    ///
    /// Used for FAUT-02 layer 1: verify that the peer certificate presented
    /// during TLS handshake matches a known server identity.
    /// O(n) scan but n is 1-2 for the Phase 7 federation setup.
    pub fn find_by_fingerprint(&self, fingerprint: &str) -> Option<ServerIdentity> {
        self.inner
            .lock()
            .unwrap()
            .values()
            .find(|id| id.tls_fingerprints.iter().any(|fp| fp == fingerprint))
            .cloned()
    }

    /// Return all cached server_id strings.
    /// Used by federation routing to discover known remote servers for relay.
    pub fn keys(&self) -> Vec<String> {
        self.inner.lock().unwrap().keys().cloned().collect()
    }
}

/// Verify TOFU (Trust On First Use) for a peer certificate fingerprint.
///
/// Per FAUT-04 (D-05): First contact pins the fingerprint. Future connections must match.
/// Per FAUT-06 (D-09): Auto-update pin on cert rotation when new fingerprint appears
/// alongside the currently pinned one in the server's tls_fingerprints array.
///
/// Returns Ok(server_id) if verification passes, Err(reason) if rejected.
pub fn verify_tofu(
    persistence: &crate::persistence::PersistenceLayer,
    remote_cache: &RemoteServerCache,
    peer_fingerprint: &str,
) -> Result<String, String> {
    // 1. Look up which server this fingerprint belongs to
    let server_identity = remote_cache
        .find_by_fingerprint(peer_fingerprint)
        .ok_or_else(|| {
            format!(
                "Peer fingerprint {} not in any known server's tls_fingerprints",
                peer_fingerprint
            )
        })?;
    let server_id = &server_identity.server_id;

    // 2. Check TOFU pin
    match persistence.get_tofu_pin(server_id)? {
        None => {
            // First contact: pin this fingerprint (D-05)
            persistence.set_tofu_pin(server_id, peer_fingerprint)?;
            tracing::info!(
                "[tofu] First contact — pinned {} -> {}",
                server_id,
                peer_fingerprint
            );
            Ok(server_id.clone())
        }
        Some(pinned) if pinned == peer_fingerprint => {
            // Exact match: OK
            Ok(server_id.clone())
        }
        Some(pinned) => {
            // Mismatch: check for cert rotation (D-09)
            let published = &server_identity.tls_fingerprints;
            if published.contains(&peer_fingerprint.to_string()) && published.contains(&pinned) {
                // Both old and new fingerprints in published array = cert rotation
                persistence.set_tofu_pin(server_id, peer_fingerprint)?;
                tracing::info!(
                    "[tofu] Cert rotation for {}: {} -> {}",
                    server_id,
                    pinned,
                    peer_fingerprint
                );
                Ok(server_id.clone())
            } else {
                // Neither rotation nor match: potential MITM
                tracing::warn!(
                    "[tofu] REJECTED {}: pinned={}, peer={}, published={:?}",
                    server_id,
                    pinned,
                    peer_fingerprint,
                    published
                );
                Err(format!(
                    "TOFU pin mismatch for {}: pinned={}, presented={}",
                    server_id, pinned, peer_fingerprint
                ))
            }
        }
    }
}

/// Axum handler: `GET /.well-known/ainm/server`
///
/// Returns this server's federation identity as JSON.
/// Cache-Control allows remote servers to cache the response for 1 hour.
pub async fn server_identity_handler(
    State(identity): State<Arc<ServerIdentity>>,
) -> impl IntoResponse {
    let body = serde_json::json!({
        "server_id": identity.server_id,
        "public_key": identity.public_key_b64,
        "relay_endpoint": identity.relay_endpoint,
        "tls_fingerprints": identity.tls_fingerprints,
        "supported_versions": identity.supported_versions,
    });
    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "application/json"),
            (header::CACHE_CONTROL, "public, max-age=3600"),
        ],
        serde_json::to_string(&body).unwrap(),
    )
}
