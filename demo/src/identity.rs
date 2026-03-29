use anyhow::Result;
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
