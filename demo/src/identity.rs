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

/// In-memory identity store for the demo.
#[derive(Clone, Default)]
pub struct IdentityStore {
    inner: Arc<Mutex<HashMap<String, IdentityRecord>>>,
}

impl IdentityStore {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(HashMap::new())),
        }
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
        self.inner.lock().unwrap().get(address).map(|r| {
            URL_SAFE_NO_PAD
                .decode(&r.public_key)
                .unwrap_or_default()
        })
    }
}

// ---------------------------------------------------------------------------
// Server-level identity for federation
// ---------------------------------------------------------------------------

/// Server-level identity for federation (distinct from participant identity).
///
/// Published at `GET /.well-known/ainm/server` so remote servers can discover
/// this server's relay endpoint, public key, and TLS fingerprint (RELY-01).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerIdentity {
    pub server_id: String,
    pub public_key_b64: String,
    pub relay_endpoint: String,
    pub tls_fingerprint: String,
    pub supported_versions: Vec<String>,
}

/// Build a `ServerIdentity` from server configuration values.
pub fn build_server_identity(
    server_id: &str,
    server_keypair: &Identity,
    federation_port: u16,
    tls_fingerprint: &str,
) -> ServerIdentity {
    ServerIdentity {
        server_id: server_id.to_string(),
        public_key_b64: server_keypair.public_key_b64(),
        relay_endpoint: format!("https://{}:{}/ainm/v1/relay", server_id, federation_port),
        tls_fingerprint: tls_fingerprint.to_string(),
        supported_versions: vec!["0.1.0".to_string()],
    }
}

/// In-memory cache of resolved remote server identities.
#[derive(Clone, Default)]
pub struct RemoteServerCache {
    inner: Arc<Mutex<HashMap<String, ServerIdentity>>>,
}

impl RemoteServerCache {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(HashMap::new())),
        }
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
    /// O(n) scan but n is 1-2 for the Phase 7 demo.
    pub fn find_by_fingerprint(&self, fingerprint: &str) -> Option<ServerIdentity> {
        self.inner
            .lock()
            .unwrap()
            .values()
            .find(|id| id.tls_fingerprint == fingerprint)
            .cloned()
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
        "tls_fingerprint": identity.tls_fingerprint,
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
