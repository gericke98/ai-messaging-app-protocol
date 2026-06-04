use crate::federation;
use crate::identity::IdentityStore;
use crate::protocol;
use crate::relay;
use crate::signing;
use anyhow::Result;
use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    extract::{Path, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use base64::Engine;
use futures_util::{SinkExt, StreamExt};
use std::net::SocketAddr;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};

/// Does this content array mention `@<localpart>` of the agent address?
fn content_mentions(content: &serde_json::Value, addr: &str) -> bool {
    content_to_plain(content).contains(&format!("@{}", addr.split('@').next().unwrap_or(addr)))
}

/// Flatten a content block array to plain text (prefers `text`, then `fallback`).
fn content_to_plain(content: &serde_json::Value) -> String {
    if let Some(arr) = content.as_array() {
        arr.iter()
            .filter_map(|b| {
                b.get("text")
                    .or_else(|| b.get("fallback"))
                    .and_then(|t| t.as_str())
            })
            .collect::<Vec<_>>()
            .join(" ")
    } else {
        content.as_str().unwrap_or("").to_string()
    }
}

/// Federation-aware routing context.
///
/// When present, the server routes messages through federation relay
/// for remote participants. When absent (legacy mode), all routing is local.
#[derive(Clone)]
pub struct FederationContext {
    pub server_id: String,
    pub thread_registry: federation::ThreadRegistry,
    pub relay_sender: Arc<relay::RelaySender>,
    pub local_delivery_tx: broadcast::Sender<String>,
    pub remote_cache: crate::identity::RemoteServerCache,
}

/// Registry of connected human WebSocket sessions.
/// Maps user_addr → list of per-connection senders (supports multiple tabs/devices).
/// Also tracks thread participation for scoped message delivery.
#[derive(Clone)]
pub struct HumanRegistry {
    connections: Arc<Mutex<HashMap<String, Vec<tokio::sync::mpsc::UnboundedSender<String>>>>>,
    /// thread_id → set of user_addrs participating in that thread.
    thread_participants: Arc<Mutex<HashMap<String, std::collections::HashSet<String>>>>,
}

impl HumanRegistry {
    pub fn new() -> Self {
        Self {
            connections: Arc::new(Mutex::new(HashMap::new())),
            thread_participants: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Register a new human connection. Returns the receiver for that connection.
    pub async fn register(&self, user_addr: &str) -> tokio::sync::mpsc::UnboundedReceiver<String> {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let mut map = self.connections.lock().await;
        map.entry(user_addr.to_string()).or_default().push(tx);
        rx
    }

    /// Unregister a human connection (removes closed senders).
    pub async fn cleanup(&self, user_addr: &str) {
        let mut map = self.connections.lock().await;
        if let Some(senders) = map.get_mut(user_addr) {
            senders.retain(|tx| !tx.is_closed());
            if senders.is_empty() {
                map.remove(user_addr);
            }
        }
    }

    /// Record that a user is participating in a thread.
    pub async fn join_thread(&self, user_addr: &str, thread_id: &str) {
        let mut map = self.thread_participants.lock().await;
        map.entry(thread_id.to_string())
            .or_default()
            .insert(user_addr.to_string());
    }

    #[allow(dead_code)]
    pub async fn send_to_user(&self, user_addr: &str, msg: &str) {
        let map = self.connections.lock().await;
        if let Some(senders) = map.get(user_addr) {
            for tx in senders {
                let _ = tx.send(msg.to_string());
            }
        }
    }

    /// Send a message to all humans participating in a specific thread.
    /// Falls back to all connected humans if the thread has no tracked participants.
    pub async fn send_to_thread(&self, thread_id: &str, msg: &str) {
        let participants = self.thread_participants.lock().await;
        if let Some(users) = participants.get(thread_id) {
            if !users.is_empty() {
                let conns = self.connections.lock().await;
                for user_addr in users {
                    if let Some(senders) = conns.get(user_addr) {
                        for tx in senders {
                            let _ = tx.send(msg.to_string());
                        }
                    }
                }
                return;
            }
        }
        drop(participants);
        // Fallback: deliver to all (thread not yet tracked, e.g. legacy mode)
        self.send_to_all(msg).await;
    }

    /// Send a message to all connected humans.
    pub async fn send_to_all(&self, msg: &str) {
        let map = self.connections.lock().await;
        for senders in map.values() {
            for tx in senders {
                let _ = tx.send(msg.to_string());
            }
        }
    }
}

/// Registry of connected agent WebSocket sessions.
/// Maps agent identity → list of per-connection senders (supports multiple instances).
/// Also stores agent card data derived from init capabilities.
#[derive(Clone)]
pub struct AgentRegistry {
    connections: Arc<Mutex<HashMap<String, Vec<tokio::sync::mpsc::UnboundedSender<String>>>>>,
    agent_cards: Arc<Mutex<HashMap<String, serde_json::Value>>>,
}

impl AgentRegistry {
    pub fn new() -> Self {
        Self {
            connections: Arc::new(Mutex::new(HashMap::new())),
            agent_cards: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Register a new agent connection with its card data. Returns the receiver.
    pub async fn register(
        &self,
        identity: &str,
        agent_card: &serde_json::Value,
    ) -> tokio::sync::mpsc::UnboundedReceiver<String> {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let mut conns = self.connections.lock().await;
        conns.entry(identity.to_string()).or_default().push(tx);
        let mut cards = self.agent_cards.lock().await;
        cards.insert(identity.to_string(), agent_card.clone());
        rx
    }

    /// Remove closed senders for an agent identity.
    pub async fn cleanup(&self, identity: &str) {
        let mut conns = self.connections.lock().await;
        if let Some(senders) = conns.get_mut(identity) {
            senders.retain(|tx| !tx.is_closed());
            if senders.is_empty() {
                conns.remove(identity);
                // Also remove the card when no connections remain
                let mut cards = self.agent_cards.lock().await;
                cards.remove(identity);
            }
        }
    }

    /// Get the cached agent card for a given identity.
    /// Retained for forthcoming per-agent card publishing on the well-known route; the
    /// client-facing `agent_card_handler` no longer routes through it (issue #2 guard).
    #[allow(dead_code)]
    pub async fn get_agent_card(&self, identity: &str) -> Option<serde_json::Value> {
        let cards = self.agent_cards.lock().await;
        cards.get(identity).cloned()
    }

    /// List all currently connected agent identities.
    #[allow(dead_code)]
    pub async fn list_agents(&self) -> Vec<String> {
        let conns = self.connections.lock().await;
        conns.keys().cloned().collect()
    }

    /// Send a message to a specific agent.
    #[allow(dead_code)]
    pub async fn send_to_agent(&self, identity: &str, msg: &str) {
        let conns = self.connections.lock().await;
        if let Some(senders) = conns.get(identity) {
            for tx in senders {
                let _ = tx.send(msg.to_string());
            }
        }
    }

    /// Broadcast a message to all connected agents.
    #[allow(dead_code)]
    pub async fn send_to_all_agents(&self, msg: &str) {
        let conns = self.connections.lock().await;
        for senders in conns.values() {
            for tx in senders {
                let _ = tx.send(msg.to_string());
            }
        }
    }
}

/// Shared server state accessible by all handlers.
#[derive(Clone)]
pub struct ServerState {
    pub identity_store: IdentityStore,
    pub agent_card: serde_json::Value,
    /// Sender for messages going to the agent.
    pub to_agent_tx: broadcast::Sender<String>,
    /// Registry of connected human sessions for scoped message delivery.
    pub human_registry: HumanRegistry,
    /// Registry of connected agent sessions with their agent card metadata.
    pub agent_registry: AgentRegistry,
    /// Active threads (thread_id list).
    pub threads: Arc<Mutex<Vec<String>>>,
    /// In-memory archival fact store keyed by fact key (fallback when no DB).
    pub archival_store: Arc<Mutex<HashMap<String, serde_json::Value>>>,
    /// Optional federation context for cross-server routing.
    pub federation: Option<FederationContext>,
    /// Optional PostgreSQL pool for production persistence.
    pub db: Option<sqlx::PgPool>,
    /// Optional sender to trigger memory compilation.
    pub compile_tx: Option<tokio::sync::mpsc::Sender<crate::compiler::CompileRequest>>,
    /// Broadcast sender for compilation notifications (worker → human clients).
    pub compile_notify_tx: Option<tokio::sync::broadcast::Sender<String>>,
    /// Email sender for account verification codes.
    pub email_sender: std::sync::Arc<dyn crate::email::EmailSender>,
    /// HMAC token signer for enrollment/auth tokens.
    pub token_signer: crate::auth_token::TokenSigner,
    /// This server's domain (used to build account addresses).
    pub server_domain: String,
    /// The provisioned hosted agent (signer + prompt), if a DB-backed agent exists.
    pub hosted_agent: Option<std::sync::Arc<crate::hosted_agent::HostedAgent>>,
    /// LLM client for the hosted agent loop.
    pub llm_client: Option<std::sync::Arc<dyn crate::llm::LlmClient>>,
    /// Curated MCP tool providers (built at startup), shared across turns.
    pub mcp_providers: std::sync::Arc<Vec<std::sync::Arc<dyn crate::tools::ToolProvider>>>,
    pub cred_cipher: Option<std::sync::Arc<crate::credentials::CredCipher>>,
    pub quota: crate::config::QuotaConfig,
    /// Graceful-shutdown coordinator. Default (never triggered) for legacy/UAT start paths;
    /// the real one is threaded in by `run_federation` for the production listener.
    pub shutdown: crate::lifecycle::ShutdownController,
}

// --- Account REST handlers (signup / verify / bind-key) ---

#[derive(serde::Deserialize)]
pub struct SignupReq { pub email: String, pub username: String }
#[derive(serde::Serialize)]
pub struct SignupResp { pub address: String }

pub async fn signup_handler(
    State(state): State<ServerState>,
    Json(req): Json<SignupReq>,
) -> Result<Json<SignupResp>, (StatusCode, String)> {
    let Some(pool) = state.db.as_ref() else {
        return Err((StatusCode::SERVICE_UNAVAILABLE, "database not configured".into()));
    };
    let username = crate::accounts::validate_username(&req.username)
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("invalid username: {e:?}")))?;
    if !req.email.contains('@') {
        return Err((StatusCode::BAD_REQUEST, "invalid email".into()));
    }
    let email_hash = crate::email::hash_email(&req.email);
    let acct = crate::accounts::create_pending_account(pool, &username, &state.server_domain, &email_hash)
        .await
        .map_err(|e| match e {
            crate::error::StorageError::Duplicate(_) =>
                (StatusCode::CONFLICT, "username or email already registered".into()),
            other => (StatusCode::INTERNAL_SERVER_ERROR, other.to_string()),
        })?;
    let code = crate::email::generate_code();
    crate::email::store_verification(pool, &email_hash, &crate::email::hash_code(&code), "signup")
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    state.email_sender.send_code(&req.email, &code, "signup").await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(SignupResp { address: acct.address }))
}

#[derive(serde::Deserialize)]
pub struct VerifyReq { pub email: String, pub code: String }
#[derive(serde::Serialize)]
pub struct VerifyResp { pub enrollment_token: String }

pub async fn verify_handler(
    State(state): State<ServerState>,
    Json(req): Json<VerifyReq>,
) -> Result<Json<VerifyResp>, (StatusCode, String)> {
    let Some(pool) = state.db.as_ref() else {
        return Err((StatusCode::SERVICE_UNAVAILABLE, "database not configured".into()));
    };
    let email_hash = crate::email::hash_email(&req.email);
    let outcome = crate::email::verify_code(pool, &email_hash, "signup", &crate::email::hash_code(&req.code))
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if outcome != crate::email::VerifyOutcome::Ok {
        return Err((StatusCode::UNAUTHORIZED, format!("{outcome:?}")));
    }
    // INVARIANT: at most one account per email_hash (enforced by the unique index
    // accounts_email_hash_idx in migration 003), so this single lookup is unambiguous.
    let acct = crate::accounts::find_account_by_email_hash(pool, &email_hash)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "account not found".into()))?;
    crate::accounts::set_email_verified(pool, acct.id).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let token = state.token_signer.issue(&acct.id.to_string(), "enrollment", crate::protocol::now_unix(), 900);
    Ok(Json(VerifyResp { enrollment_token: token }))
}

#[derive(serde::Deserialize)]
pub struct BindKeyReq { pub enrollment_token: String, pub public_key: String, pub device_label: Option<String> }

pub async fn bind_key_handler(
    State(state): State<ServerState>,
    Json(req): Json<BindKeyReq>,
) -> Result<StatusCode, (StatusCode, String)> {
    let Some(pool) = state.db.as_ref() else {
        return Err((StatusCode::SERVICE_UNAVAILABLE, "database not configured".into()));
    };
    let account_id_str = state.token_signer
        .verify(&req.enrollment_token, "enrollment", crate::protocol::now_unix())
        .map_err(|e| (StatusCode::UNAUTHORIZED, format!("{e:?}")))?;
    let account_id = uuid::Uuid::parse_str(&account_id_str)
        .map_err(|_| (StatusCode::BAD_REQUEST, "bad token subject".into()))?;
    match crate::accounts::insert_account_key(pool, account_id, &req.public_key, req.device_label.as_deref()).await {
        Ok(()) => Ok(StatusCode::NO_CONTENT),
        // Idempotent: the SAME key already bound to the SAME account (e.g. onboarding
        // retried on an already-enrolled device) is a no-op, not a conflict. A key bound
        // to a DIFFERENT account is still a genuine conflict (global unique index).
        Err(crate::error::StorageError::Duplicate(_)) => {
            let already_ours = crate::accounts::key_bound_to_account(pool, account_id, &req.public_key)
                .await
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            if already_ours {
                Ok(StatusCode::NO_CONTENT)
            } else {
                Err((StatusCode::CONFLICT, "key already bound".into()))
            }
        }
        Err(other) => Err((StatusCode::INTERNAL_SERVER_ERROR, other.to_string())),
    }
}

/// Resolve the calling account + presented-token hash from the `Authorization: Bearer
/// <session-token>` header, validating the DB-backed session (reject revoked/expired,
/// slide on use). Replaces the old enrollment-token check.
async fn caller_from_bearer(
    state: &ServerState,
    headers: &HeaderMap,
) -> Result<crate::auth_session::Caller, (StatusCode, String)> {
    let pool = state.db.as_ref().ok_or((StatusCode::SERVICE_UNAVAILABLE, "no database".into()))?;
    let raw = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .ok_or((StatusCode::UNAUTHORIZED, "missing Authorization header".into()))?;
    let token = raw.strip_prefix("Bearer ").ok_or((StatusCode::UNAUTHORIZED, "expected Bearer token".into()))?;
    match crate::auth_session::authenticate(pool, token, chrono::Utc::now()).await {
        Ok(Some(caller)) => Ok(caller),
        Ok(None) => Err((StatusCode::UNAUTHORIZED, "invalid session".into())),
        Err(_) => Err((StatusCode::INTERNAL_SERVER_ERROR, "session lookup failed".into())),
    }
}

/// Account-id-only convenience for endpoints that don't need the token hash.
async fn account_from_bearer(
    state: &ServerState,
    headers: &HeaderMap,
) -> Result<uuid::Uuid, (StatusCode, String)> {
    Ok(caller_from_bearer(state, headers).await?.account_id)
}

#[derive(serde::Deserialize)]
pub struct LlmCredentialReq {
    pub api_key: String,
    pub model: Option<String>,
    pub base_url: Option<String>,
}

pub async fn llm_credential_put_handler(
    State(state): State<ServerState>,
    headers: HeaderMap,
    Json(req): Json<LlmCredentialReq>,
) -> Result<StatusCode, (StatusCode, String)> {
    let account_id = account_from_bearer(&state, &headers).await?;
    let pool = state.db.clone().ok_or((StatusCode::SERVICE_UNAVAILABLE, "no database".into()))?;
    let cipher = state
        .cred_cipher
        .clone()
        .ok_or((StatusCode::SERVICE_UNAVAILABLE, "byo_not_configured".into()))?;
    let (ct, nonce) = cipher.encrypt(&req.api_key).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    crate::credentials::upsert_byo_key(&pool, account_id, &ct, &nonce, req.model.as_deref(), req.base_url.as_deref())
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn llm_credential_delete_handler(
    State(state): State<ServerState>,
    headers: HeaderMap,
) -> Result<StatusCode, (StatusCode, String)> {
    let account_id = account_from_bearer(&state, &headers).await?;
    let pool = state.db.clone().ok_or((StatusCode::SERVICE_UNAVAILABLE, "no database".into()))?;
    crate::credentials::clear_byo_key(&pool, account_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn usage_get_handler(
    State(state): State<ServerState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let account_id = account_from_bearer(&state, &headers).await?;
    let pool = state.db.clone().ok_or((StatusCode::SERVICE_UNAVAILABLE, "no database".into()))?;
    let account = crate::accounts::find_account_by_id(&pool, account_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "account not found".into()))?;
    let settings = crate::credentials::get_settings(&pool, account_id).await.ok().flatten();
    let period = crate::quota::current_period(chrono::Utc::now());
    let used = crate::quota::current_usage(&pool, &account.address, &period).await.unwrap_or(0);
    let budget = crate::quota::resolve_budget(
        settings.as_ref().and_then(|s| s.monthly_token_budget),
        state.quota.monthly_token_budget,
    );
    let byo_key_set = settings.as_ref().map(|s| s.api_key_ciphertext.is_some()).unwrap_or(false);
    Ok(Json(serde_json::json!({
        "period": period,
        "used_tokens": used,
        "budget": budget,
        "remaining": (budget - used).max(0),
        "byo_key_set": byo_key_set,
    })))
}

#[derive(serde::Deserialize)]
pub struct MintTokenReq {
    pub identity: String,
    pub timestamp: String,
    pub signature: String,
    pub device_label: Option<String>,
}
#[derive(serde::Serialize)]
pub struct MintTokenResp {
    pub token: String,
    pub session_id: uuid::Uuid,
    pub expires_at: chrono::DateTime<chrono::Utc>,
}

/// `POST /accounts/auth/token` — mint a session by proving possession of the account's
/// bound Ed25519 key (domain-separated challenge). Uniform 401 on any auth failure.
pub async fn auth_token_mint_handler(
    State(state): State<ServerState>,
    Json(req): Json<MintTokenReq>,
) -> Result<Json<MintTokenResp>, (StatusCode, String)> {
    let pool = state.db.as_ref().ok_or((StatusCode::SERVICE_UNAVAILABLE, "no database".into()))?;
    let unauthorized = || (StatusCode::UNAUTHORIZED, "authentication failed".to_string());

    let account = crate::accounts::find_account_by_address(pool, &req.identity)
        .await.map_err(|_| unauthorized())?.ok_or_else(unauthorized)?;
    let key = crate::accounts::find_active_key(pool, account.id)
        .await.map_err(|_| unauthorized())?.ok_or_else(unauthorized)?;
    let pk_bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(&key.public_key).map_err(|_| unauthorized())?;

    if !crate::signing::verify_auth_challenge(
        &pk_bytes, &req.identity, &req.timestamp, &req.signature, chrono::Utc::now(), 300,
    ) {
        return Err(unauthorized());
    }

    let now = chrono::Utc::now();
    let (token, hash) = crate::auth_session::generate_token();
    let expires_at = crate::auth_session::next_expiry(now, now);
    let session_id = crate::auth_session::insert(
        pool, account.id, &hash, req.device_label.as_deref(), now, expires_at,
    ).await.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(MintTokenResp { token, session_id, expires_at }))
}

#[derive(serde::Serialize)]
pub struct SessionView {
    pub id: uuid::Uuid,
    pub device_label: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub last_used_at: chrono::DateTime<chrono::Utc>,
    pub expires_at: chrono::DateTime<chrono::Utc>,
    pub current: bool,
}

/// `GET /accounts/auth/sessions` — list the caller's active sessions (current flagged).
pub async fn sessions_list_handler(
    State(state): State<ServerState>,
    headers: HeaderMap,
) -> Result<Json<Vec<SessionView>>, (StatusCode, String)> {
    let caller = caller_from_bearer(&state, &headers).await?;
    let pool = state.db.clone().ok_or((StatusCode::SERVICE_UNAVAILABLE, "no database".into()))?;
    let now = chrono::Utc::now();
    let sessions = crate::auth_session::list_active(&pool, caller.account_id, now)
        .await.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    // `list_active` rows don't carry the token hash, so resolve the current session's id
    // by looking it up from the caller's presented-token hash, then flag the matching row.
    let current_id = crate::auth_session::find_active_by_hash(&pool, &caller.token_hash, now)
        .await.ok().flatten().map(|s| s.id);
    let views = sessions.into_iter().map(|s| SessionView {
        current: Some(s.id) == current_id,
        id: s.id,
        device_label: s.device_label,
        created_at: s.created_at,
        last_used_at: s.last_used_at,
        expires_at: s.expires_at,
    }).collect();
    Ok(Json(views))
}

/// `DELETE /accounts/auth/sessions/{id}` — revoke one owned session.
pub async fn session_revoke_handler(
    State(state): State<ServerState>,
    headers: HeaderMap,
    Path(session_id): Path<uuid::Uuid>,
) -> Result<StatusCode, (StatusCode, String)> {
    let caller = caller_from_bearer(&state, &headers).await?;
    let pool = state.db.clone().ok_or((StatusCode::SERVICE_UNAVAILABLE, "no database".into()))?;
    let ok = crate::auth_session::revoke_one(&pool, caller.account_id, session_id, chrono::Utc::now())
        .await.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if ok { Ok(StatusCode::NO_CONTENT) } else { Err((StatusCode::NOT_FOUND, "no such session".into())) }
}

#[derive(serde::Serialize)]
pub struct RevokeOthersResp { pub revoked: u64 }

/// `DELETE /accounts/auth/sessions/others` — revoke all the caller's sessions but the current.
pub async fn sessions_revoke_others_handler(
    State(state): State<ServerState>,
    headers: HeaderMap,
) -> Result<Json<RevokeOthersResp>, (StatusCode, String)> {
    let caller = caller_from_bearer(&state, &headers).await?;
    let pool = state.db.clone().ok_or((StatusCode::SERVICE_UNAVAILABLE, "no database".into()))?;
    let revoked = crate::auth_session::revoke_others(&pool, caller.account_id, &caller.token_hash, chrono::Utc::now())
        .await.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(RevokeOthersResp { revoked }))
}

#[derive(serde::Deserialize)]
pub struct StoreEscrowReq {
    pub enrollment_token: String,
    pub ciphertext_b64: String,
    pub kdf_params: crate::escrow::KdfParams,
    pub oprf_key_b64: String,
}

pub async fn store_escrow_handler(
    State(state): State<ServerState>,
    Json(req): Json<StoreEscrowReq>,
) -> Result<StatusCode, (StatusCode, String)> {
    let b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD;
    let Some(pool) = state.db.as_ref() else {
        return Err((StatusCode::SERVICE_UNAVAILABLE, "database not configured".into()));
    };
    let account_id_str = state.token_signer
        .verify(&req.enrollment_token, "enrollment", crate::protocol::now_unix())
        .map_err(|e| (StatusCode::UNAUTHORIZED, format!("{e:?}")))?;
    let account_id = uuid::Uuid::parse_str(&account_id_str)
        .map_err(|_| (StatusCode::BAD_REQUEST, "bad token subject".into()))?;
    let ciphertext = b64.decode(&req.ciphertext_b64).map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    let oprf_key = b64.decode(&req.oprf_key_b64).map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    crate::escrow::store_escrow(pool, account_id, &ciphertext, &req.kdf_params, &oprf_key)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    crate::accounts::set_status(pool, account_id, "active")
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(serde::Deserialize)]
pub struct RecoverStartReq { pub email: String }

pub async fn recover_start_handler(
    State(state): State<ServerState>,
    Json(req): Json<RecoverStartReq>,
) -> Result<StatusCode, (StatusCode, String)> {
    let Some(pool) = state.db.as_ref() else {
        return Err((StatusCode::SERVICE_UNAVAILABLE, "database not configured".into()));
    };
    let email_hash = crate::email::hash_email(&req.email);
    // Send a code only if the account exists; respond 204 either way (don't leak existence).
    if crate::accounts::find_account_by_email_hash(pool, &email_hash).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?.is_some()
    {
        let code = crate::email::generate_code();
        crate::email::store_verification(pool, &email_hash, &crate::email::hash_code(&code), "recovery")
            .await.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        state.email_sender.send_code(&req.email, &code, "recovery").await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    }
    Ok(StatusCode::NO_CONTENT)
}

#[derive(serde::Deserialize)]
pub struct RecoverVerifyReq { pub email: String, pub code: String }
#[derive(serde::Serialize)]
pub struct RecoverVerifyResp { pub recovery_token: String }

pub async fn recover_verify_handler(
    State(state): State<ServerState>,
    Json(req): Json<RecoverVerifyReq>,
) -> Result<Json<RecoverVerifyResp>, (StatusCode, String)> {
    let Some(pool) = state.db.as_ref() else {
        return Err((StatusCode::SERVICE_UNAVAILABLE, "database not configured".into()));
    };
    let email_hash = crate::email::hash_email(&req.email);
    let outcome = crate::email::verify_code(pool, &email_hash, "recovery", &crate::email::hash_code(&req.code))
        .await.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if outcome != crate::email::VerifyOutcome::Ok {
        return Err((StatusCode::UNAUTHORIZED, format!("{outcome:?}")));
    }
    let acct = crate::accounts::find_account_by_email_hash(pool, &email_hash).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "account not found".into()))?;
    let token = state.token_signer.issue(&acct.id.to_string(), "recovery", crate::protocol::now_unix(), 900);
    Ok(Json(RecoverVerifyResp { recovery_token: token }))
}

#[derive(serde::Deserialize)]
pub struct EscrowFetchReq { pub recovery_token: String, pub blinded_point: String }
#[derive(serde::Serialize)]
pub struct EscrowFetchResp { pub evaluated_point: String, pub ciphertext_b64: String, pub kdf_params: crate::escrow::KdfParams }

pub async fn escrow_fetch_handler(
    State(state): State<ServerState>,
    Json(req): Json<EscrowFetchReq>,
) -> Result<Json<EscrowFetchResp>, (StatusCode, String)> {
    let b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD;
    let Some(pool) = state.db.as_ref() else {
        return Err((StatusCode::SERVICE_UNAVAILABLE, "database not configured".into()));
    };
    let account_id_str = state.token_signer
        .verify(&req.recovery_token, "recovery", crate::protocol::now_unix())
        .map_err(|e| (StatusCode::UNAUTHORIZED, format!("{e:?}")))?;
    let account_id = uuid::Uuid::parse_str(&account_id_str)
        .map_err(|_| (StatusCode::BAD_REQUEST, "bad token subject".into()))?;
    match crate::escrow::fetch_and_evaluate(pool, account_id, &req.blinded_point).await {
        Ok(f) => Ok(Json(EscrowFetchResp {
            evaluated_point: f.evaluated_b64,
            ciphertext_b64: b64.encode(&f.ciphertext),
            kdf_params: f.kdf_params,
        })),
        Err(crate::escrow::FetchError::Destroyed) => Err((StatusCode::GONE, "recovery blob destroyed".into())),
        Err(crate::escrow::FetchError::NotFound) => Err((StatusCode::NOT_FOUND, "no escrow".into())),
        Err(crate::escrow::FetchError::Crypto(e)) => Err((StatusCode::BAD_REQUEST, e)),
        Err(crate::escrow::FetchError::Db(e)) => Err((StatusCode::INTERNAL_SERVER_ERROR, e)),
    }
}

#[derive(serde::Deserialize)]
pub struct RecoverParamsReq { pub recovery_token: String }
#[derive(serde::Serialize)]
pub struct RecoverParamsResp { pub kdf_params: crate::escrow::KdfParams }

/// `POST /accounts/recover/params` — return the Argon2 kdf_params (salt) for a recovering
/// account WITHOUT consuming a guess. On a fresh device the client needs the salt to blind a
/// PIN before it can fetch; doing that via `/escrow/fetch` would waste an attempt. This is
/// recovery-token authenticated, read-only, and never touches `eval_attempts`.
pub async fn recover_params_handler(
    State(state): State<ServerState>,
    Json(req): Json<RecoverParamsReq>,
) -> Result<Json<RecoverParamsResp>, (StatusCode, String)> {
    let Some(pool) = state.db.as_ref() else {
        return Err((StatusCode::SERVICE_UNAVAILABLE, "database not configured".into()));
    };
    let aid = state.token_signer
        .verify(&req.recovery_token, "recovery", crate::protocol::now_unix())
        .map_err(|e| (StatusCode::UNAUTHORIZED, format!("{e:?}")))?;
    let account_id = uuid::Uuid::parse_str(&aid)
        .map_err(|_| (StatusCode::BAD_REQUEST, "bad token subject".into()))?;
    match crate::escrow::get_kdf_params(pool, account_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    {
        Some((_, true)) => Err((StatusCode::GONE, "recovery blob destroyed".into())),
        Some((kdf, false)) => Ok(Json(RecoverParamsResp { kdf_params: kdf })),
        None => Err((StatusCode::NOT_FOUND, "no escrow".into())),
    }
}

/// `GET /.well-known/ainm-identity/{username}` — identity resolution endpoint.
///
/// Resolves a local username to its full AINM address and active public key(s).
/// Federation-ready: a server-signed attestation MAY be attached so remote servers
/// can verify the binding without a live connection. Cross-server resolution is
/// NOT exercised in v1.
pub async fn well_known_identity_handler(
    State(state): State<ServerState>,
    Path(username): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let Some(pool) = state.db.as_ref() else {
        return Err((StatusCode::SERVICE_UNAVAILABLE, "database not configured".into()));
    };
    let address = format!("{username}@{}", state.server_domain);
    let acct = crate::accounts::find_account_by_address(pool, &address).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "unknown account".into()))?;
    let key = crate::accounts::find_active_key(pool, acct.id).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let public_keys: Vec<String> = key.map(|k| vec![k.public_key]).unwrap_or_default();

    let body = serde_json::json!({
        "address": address,
        "public_keys": public_keys,
    });

    // TODO(B): sign attestation once the server key is exposed in ServerState.
    // The server's Ed25519 keypair currently lives only in federation::FederationState
    // (built in main.rs) and is not threaded into ServerState; cross-server resolution
    // is untested in v1, so the attestation is omitted rather than wired through all
    // three router builders + main.rs construction sites.

    Ok(Json(body))
}

/// Start the Axum WebSocket server on a random available port (legacy mode).
/// Returns the bound address and the server task handle.
pub async fn start(
    identity_store: IdentityStore,
    agent_card: serde_json::Value,
) -> Result<(SocketAddr, tokio::task::JoinHandle<()>)> {
    let (to_agent_tx, _) = broadcast::channel::<String>(256);

    let state = ServerState {
        identity_store,
        agent_card,
        to_agent_tx,
        human_registry: HumanRegistry::new(),
        agent_registry: AgentRegistry::new(),
        threads: Arc::new(Mutex::new(Vec::new())),
        archival_store: Arc::new(Mutex::new(HashMap::new())),
        federation: None,
        db: None,
        compile_tx: None,
        compile_notify_tx: None,
        email_sender: std::sync::Arc::new(crate::email::MockEmailSender::new()),
        token_signer: crate::auth_token::TokenSigner::new(b"dev-insecure-secret"),
        server_domain: "ainm.local".to_string(),
        hosted_agent: None,
        llm_client: None,
        mcp_providers: std::sync::Arc::new(vec![]),
        cred_cipher: None,
        quota: crate::config::QuotaConfig::default(),
        shutdown: crate::lifecycle::ShutdownController::new(),
    };

    let shutdown_rx = state.shutdown.subscribe();
    let app = Router::new()
        .route("/ws", get(ws_upgrade_handler))
        .route(
            "/.well-known/ainm/{agent_name}/agent-card.json",
            get(agent_card_handler),
        )
        .route("/accounts/signup", axum::routing::post(signup_handler))
        .route("/accounts/verify", axum::routing::post(verify_handler))
        .route("/accounts/keys", axum::routing::post(bind_key_handler))
        .route("/accounts/escrow", axum::routing::post(store_escrow_handler))
        .route("/accounts/recover/start", axum::routing::post(recover_start_handler))
        .route("/accounts/recover/verify", axum::routing::post(recover_verify_handler))
        .route("/accounts/escrow/fetch", axum::routing::post(escrow_fetch_handler))
        .route("/accounts/recover/params", axum::routing::post(recover_params_handler))
        .route("/accounts/llm-credential", axum::routing::put(llm_credential_put_handler).delete(llm_credential_delete_handler))
        .route("/accounts/usage", axum::routing::get(usage_get_handler))
        .route("/accounts/auth/token", axum::routing::post(auth_token_mint_handler))
        .route("/accounts/auth/sessions", axum::routing::get(sessions_list_handler))
        .route("/accounts/auth/sessions/others", axum::routing::delete(sessions_revoke_others_handler))
        .route("/accounts/auth/sessions/{id}", axum::routing::delete(session_revoke_handler))
        .route("/.well-known/ainm-identity/{username}", get(well_known_identity_handler))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;

    let handle = tokio::spawn(async move {
        axum::serve(listener, app)
            .with_graceful_shutdown(crate::lifecycle::wait_for_shutdown(shutdown_rx))
            .await
            .ok();
    });

    Ok((addr, handle))
}

/// Start the Axum WebSocket server on a specific port with `/health` + `/health/live`
/// routes registered (Phase 14-02 UAT automation harness).
///
/// Mirrors the legacy `start()` path (no federation, no DB, no compilation worker) but:
/// - Binds `127.0.0.1:{port}` instead of a random port.
/// - Registers `/health` and `/health/live` so `make uat` can poll for readiness.
///
/// Used exclusively by `main::run_uat` behind the `--uat` CLI flag; production code
/// paths are unaffected.
pub async fn start_with_port(
    identity_store: IdentityStore,
    agent_card: serde_json::Value,
    port: u16,
) -> Result<(SocketAddr, tokio::task::JoinHandle<()>)> {
    let (to_agent_tx, _) = broadcast::channel::<String>(256);

    let state = ServerState {
        identity_store,
        agent_card,
        to_agent_tx,
        human_registry: HumanRegistry::new(),
        agent_registry: AgentRegistry::new(),
        threads: Arc::new(Mutex::new(Vec::new())),
        archival_store: Arc::new(Mutex::new(HashMap::new())),
        federation: None,
        db: None,
        compile_tx: None,
        compile_notify_tx: None,
        email_sender: std::sync::Arc::new(crate::email::MockEmailSender::new()),
        token_signer: crate::auth_token::TokenSigner::new(b"dev-insecure-secret"),
        server_domain: "ainm.local".to_string(),
        hosted_agent: None,
        llm_client: None,
        mcp_providers: std::sync::Arc::new(vec![]),
        cred_cipher: None,
        quota: crate::config::QuotaConfig::default(),
        shutdown: crate::lifecycle::ShutdownController::new(),
    };

    let shutdown_rx = state.shutdown.subscribe();
    let app = Router::new()
        .route("/ws", get(ws_upgrade_handler))
        .route(
            "/.well-known/ainm/{agent_name}/agent-card.json",
            get(agent_card_handler),
        )
        .route("/health", get(|| async { StatusCode::OK }))
        .route("/health/live", get(|| async { StatusCode::OK }))
        .route("/accounts/signup", axum::routing::post(signup_handler))
        .route("/accounts/verify", axum::routing::post(verify_handler))
        .route("/accounts/keys", axum::routing::post(bind_key_handler))
        .route("/accounts/escrow", axum::routing::post(store_escrow_handler))
        .route("/accounts/recover/start", axum::routing::post(recover_start_handler))
        .route("/accounts/recover/verify", axum::routing::post(recover_verify_handler))
        .route("/accounts/escrow/fetch", axum::routing::post(escrow_fetch_handler))
        .route("/accounts/recover/params", axum::routing::post(recover_params_handler))
        .route("/accounts/llm-credential", axum::routing::put(llm_credential_put_handler).delete(llm_credential_delete_handler))
        .route("/accounts/usage", axum::routing::get(usage_get_handler))
        .route("/accounts/auth/token", axum::routing::post(auth_token_mint_handler))
        .route("/accounts/auth/sessions", axum::routing::get(sessions_list_handler))
        .route("/accounts/auth/sessions/others", axum::routing::delete(sessions_revoke_others_handler))
        .route("/accounts/auth/sessions/{id}", axum::routing::delete(session_revoke_handler))
        .route("/.well-known/ainm-identity/{username}", get(well_known_identity_handler))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{}", port)).await?;
    let addr = listener.local_addr()?;

    let handle = tokio::spawn(async move {
        axum::serve(listener, app)
            .with_graceful_shutdown(crate::lifecycle::wait_for_shutdown(shutdown_rx))
            .await
            .ok();
    });

    Ok((addr, handle))
}

/// Start the Axum WebSocket server with federation support on a specified port.
/// Returns the bound address and the server task handle.
// builder takes many collaborators; v1 keeps the flat signature
#[allow(clippy::too_many_arguments)]
pub async fn start_federation(
    identity_store: IdentityStore,
    agent_card: serde_json::Value,
    federation: Option<FederationContext>,
    port: u16,
    db: Option<sqlx::PgPool>,
    compile_tx: Option<tokio::sync::mpsc::Sender<crate::compiler::CompileRequest>>,
    compile_notify_tx: Option<tokio::sync::broadcast::Sender<String>>,
    email_sender: std::sync::Arc<dyn crate::email::EmailSender>,
    token_signer: crate::auth_token::TokenSigner,
    server_domain: String,
    hosted_agent: Option<std::sync::Arc<crate::hosted_agent::HostedAgent>>,
    llm_client: Option<std::sync::Arc<dyn crate::llm::LlmClient>>,
    mcp_providers: std::sync::Arc<Vec<std::sync::Arc<dyn crate::tools::ToolProvider>>>,
    cred_cipher: Option<std::sync::Arc<crate::credentials::CredCipher>>,
    quota: crate::config::QuotaConfig,
    shutdown: crate::lifecycle::ShutdownController,
) -> Result<(SocketAddr, tokio::task::JoinHandle<()>)> {
    let (to_agent_tx, _) = broadcast::channel::<String>(256);

    let shutdown_rx = shutdown.subscribe();
    let state = ServerState {
        identity_store,
        agent_card,
        to_agent_tx,
        human_registry: HumanRegistry::new(),
        agent_registry: AgentRegistry::new(),
        threads: Arc::new(Mutex::new(Vec::new())),
        archival_store: Arc::new(Mutex::new(HashMap::new())),
        federation,
        db,
        compile_tx,
        compile_notify_tx,
        email_sender,
        token_signer,
        server_domain,
        hosted_agent,
        llm_client,
        mcp_providers,
        cred_cipher,
        quota,
        shutdown: shutdown.clone(),
    };

    let app = Router::new()
        .route("/ws", get(ws_upgrade_handler))
        .route(
            // Reference: single agent -- production would look up by agent_name
            "/.well-known/ainm/{agent_name}/agent-card.json",
            get(agent_card_handler),
        )
        .route("/health/live", get(|| async { StatusCode::OK }))
        .route("/health/ready", get(health_ready_handler))
        .route("/accounts/signup", axum::routing::post(signup_handler))
        .route("/accounts/verify", axum::routing::post(verify_handler))
        .route("/accounts/keys", axum::routing::post(bind_key_handler))
        .route("/accounts/escrow", axum::routing::post(store_escrow_handler))
        .route("/accounts/recover/start", axum::routing::post(recover_start_handler))
        .route("/accounts/recover/verify", axum::routing::post(recover_verify_handler))
        .route("/accounts/escrow/fetch", axum::routing::post(escrow_fetch_handler))
        .route("/accounts/recover/params", axum::routing::post(recover_params_handler))
        .route("/accounts/llm-credential", axum::routing::put(llm_credential_put_handler).delete(llm_credential_delete_handler))
        .route("/accounts/usage", axum::routing::get(usage_get_handler))
        .route("/accounts/auth/token", axum::routing::post(auth_token_mint_handler))
        .route("/accounts/auth/sessions", axum::routing::get(sessions_list_handler))
        .route("/accounts/auth/sessions/others", axum::routing::delete(sessions_revoke_others_handler))
        .route("/accounts/auth/sessions/{id}", axum::routing::delete(session_revoke_handler))
        .route("/.well-known/ainm-identity/{username}", get(well_known_identity_handler))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port)).await?;
    let addr = listener.local_addr()?;

    let handle = tokio::spawn(async move {
        axum::serve(listener, app)
            .with_graceful_shutdown(crate::lifecycle::wait_for_shutdown(shutdown_rx))
            .await
            .ok();
    });

    Ok((addr, handle))
}

/// The agent card this server publishes for `agent_name`, or `None` (→ 404).
///
/// Resolution order:
/// 1. **Hosted assistant:** if a hosted agent is provisioned and its address local-part
///    matches `agent_name`, serve its freshly built canonical card.
/// 2. **Static card:** the static `state.agent_card` is published only when its own declared
///    address local-part matches `agent_name`.
///
/// A null / identity-less card with no matching hosted agent publishes nothing, so every name
/// 404s. This keeps the well-known route from returning a card under an arbitrary (typo'd or
/// not-yet-connected) agent name. Per-agent card publishing for live connector agents is a
/// separate, larger change and is intentionally NOT handled here.
fn published_card_for(state: &ServerState, agent_name: &str) -> Option<serde_json::Value> {
    // (1) Hosted assistant: build its canonical card on demand.
    if let Some(agent) = &state.hosted_agent {
        let local = agent.address.split('@').next().unwrap_or("");
        if !local.is_empty() && local == agent_name {
            return Some(crate::agent_card::build_assistant_card(&agent.address));
        }
    }

    // (2) Static card: serve only under its own declared identity.
    let card = &state.agent_card;
    let addr = card
        .get("address")
        .and_then(|v| v.as_str())
        .or_else(|| {
            card.get("ainm")
                .and_then(|a| a.get("address"))
                .and_then(|v| v.as_str())
        })?;
    let local = addr.split('@').next().unwrap_or("");
    if !local.is_empty() && local == agent_name {
        Some(card.clone())
    } else {
        None
    }
}

/// Agent Card HTTP endpoint with ETag caching (per DISC-05).
/// Serves the static card only when `agent_name` matches the server's own published identity.
async fn agent_card_handler(
    Path(agent_name): Path<String>,
    headers: HeaderMap,
    State(state): State<ServerState>,
) -> Response {
    // Serve a card ONLY when `agent_name` matches the agent this server actually publishes
    // (its own hosted/static agent identity). Unknown names 404 rather than leaking the static
    // card under an arbitrary name (which would mislabel a typo'd or offline agent).
    let Some(card) = published_card_for(&state, &agent_name) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let body = serde_json::to_vec(&card).unwrap();
    let hash = ring::digest::digest(&ring::digest::SHA256, &body);
    let etag_value = format!(
        "\"{}\"",
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(hash.as_ref())
    );

    // Check If-None-Match
    if let Some(inm) = headers.get(header::IF_NONE_MATCH) {
        if let Ok(inm_str) = inm.to_str() {
            if inm_str == etag_value {
                return (
                    StatusCode::NOT_MODIFIED,
                    [(header::ETAG, etag_value)],
                )
                    .into_response();
            }
        }
    }

    // Return 200 with body and ETag
    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "application/json".to_string()),
            (header::ETAG, etag_value),
        ],
        body,
    )
        .into_response()
}

/// Health readiness check — verifies database connectivity.
async fn health_ready_handler(State(state): State<ServerState>) -> StatusCode {
    if let Some(ref db) = state.db {
        if crate::db::health_check(db).await {
            StatusCode::OK
        } else {
            StatusCode::SERVICE_UNAVAILABLE
        }
    } else {
        // No DB configured — still healthy (in-memory mode)
        StatusCode::OK
    }
}

/// WebSocket upgrade handler.
async fn ws_upgrade_handler(
    ws: WebSocketUpgrade,
    State(state): State<ServerState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_ws_connection(socket, state))
}

/// Handle a single WebSocket connection.
async fn handle_ws_connection(socket: WebSocket, state: ServerState) {
    let (ws_sender, mut ws_receiver) = socket.split();
    let ws_sender = Arc::new(Mutex::new(ws_sender));

    // Wait for ainm.initialize request
    let init_text = match ws_receiver.next().await {
        Some(Ok(Message::Text(text))) => text.to_string(),
        _ => return,
    };

    let init_req: serde_json::Value = match serde_json::from_str(&init_text) {
        Ok(v) => v,
        Err(_) => return,
    };

    let req_id = init_req.get("id").cloned().unwrap_or(serde_json::json!(1));
    let from_addr = init_req
        .get("params")
        .and_then(|p| p.get("identity"))
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();

    // --- Anti-spoof: verify the claimed address against its REGISTERED key (sub-project A) ---
    // Only enforced when a DB is configured; in-memory / legacy mode preserves prior behavior.
    if let Some(pool) = state.db.clone() {
        let signature = init_req
            .get("params").and_then(|p| p.get("signature")).and_then(|v| v.as_str())
            .unwrap_or("");
        let timestamp = init_req
            .get("params").and_then(|p| p.get("timestamp")).and_then(|v| v.as_str())
            .unwrap_or("");

        // Send a JSON-RPC auth error (-32021) using the file's concrete sink, then close the
        // connection. Inlined (vs. a helper fn) to reuse the exact send pattern used elsewhere
        // in this handler for the "Not initialized" path.
        macro_rules! reject_conn {
            ($msg:expr) => {{
                let err = protocol::new_error(req_id.clone(), -32021, $msg);
                let mut s = ws_sender.lock().await;
                let _ = s.send(Message::Text(serde_json::to_string(&err).unwrap().into())).await;
                return;
            }};
        }

        let account = match crate::accounts::find_account_by_address(&pool, &from_addr).await {
            Ok(Some(a)) => a,
            _ => reject_conn!("unknown address"),
        };
        if account.status != "active" {
            reject_conn!("account not active");
        }
        let registered_pk_b64 = match crate::accounts::find_active_key(&pool, account.id).await {
            Ok(Some(k)) => k.public_key,
            _ => reject_conn!("no registered key"),
        };
        let pk_bytes = match base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(&registered_pk_b64) {
            Ok(b) => b,
            Err(_) => reject_conn!("corrupt key"),
        };
        if !crate::signing::verify_init(&pk_bytes, &from_addr, timestamp, signature, chrono::Utc::now(), 300) {
            reject_conn!("signature does not match registered key");
        }
        tracing::info!("[handshake] verified binding for {from_addr}");
    }

    // Send ainm.initialized response
    let init_response = protocol::new_response(
        req_id,
        serde_json::json!({
            "protocolVersion": "0.1.0",
            "serverInfo": {
                "name": "ainm.local",
                "version": "0.1.0"
            },
            "capabilities": {
                "streaming": true,
                "subThreads": true,
                "storeAndForward": false
            },
            "nonce": protocol::new_message_id()
        }),
    );
    {
        let resp_text = serde_json::to_string(&init_response).unwrap();
        let mut sender = ws_sender.lock().await;
        if sender.send(Message::Text(resp_text.into())).await.is_err() {
            return;
        }
    }

    // Wait for ainm.initialized notification before allowing any methods (transport.md Section 4.2)
    loop {
        match ws_receiver.next().await {
            Some(Ok(Message::Text(text))) => {
                let text_str = text.to_string();
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&text_str) {
                    let method = parsed.get("method").and_then(|m| m.as_str()).unwrap_or("");
                    if method == "ainm.initialized" {
                        tracing::info!("Handshake complete for: {}", from_addr);
                        break;
                    }
                    // Any other method before initialized -> -32021 (transport.md Section 7)
                    if let Some(req_id) = parsed.get("id").cloned() {
                        let err = protocol::new_error(req_id, -32021, "Not initialized");
                        let mut sender = ws_sender.lock().await;
                        let _ = sender.send(Message::Text(
                            serde_json::to_string(&err).unwrap().into(),
                        )).await;
                    }
                    // Notifications without id: silently drop per JSON-RPC spec
                }
            }
            _ => return, // Connection closed or binary frame
        }
    }

    // Determine if this is an agent or human based on participant_type in capabilities.
    // Defaults to "human" for backward compatibility when the field is absent.
    let is_agent = init_req
        .get("params")
        .and_then(|p| p.get("capabilities"))
        .and_then(|c| c.get("participant_type"))
        .and_then(|t| t.as_str())
        == Some("agent");

    if is_agent {
        handle_agent_connection(ws_sender, ws_receiver, state, from_addr, &init_req).await;
    } else {
        handle_human_connection(ws_sender, ws_receiver, state, from_addr).await;
    }
}

/// Store a message to PostgreSQL and optionally trigger memory compilation.
///
/// Messages are stored for history. Compilation is triggered after agent responses
/// (which represent completed exchanges with the richest extractable knowledge).
/// A minimum message count threshold prevents compilation on trivial threads.
async fn store_and_maybe_compile(
    db: &sqlx::PgPool,
    compile_tx: &Option<tokio::sync::mpsc::Sender<crate::compiler::CompileRequest>>,
    thread_id: &str,
    from_addr: &str,
    content: &serde_json::Value,
    trigger_compilation: bool,
) {
    // 0. Ensure thread exists (FK constraint on messages table)
    let _ = sqlx::query(
        "INSERT INTO threads (id, origin_domain) VALUES ($1, $2) ON CONFLICT DO NOTHING",
    )
    .bind(thread_id)
    .bind("local")
    .execute(db)
    .await;

    // 1. Store message to PostgreSQL
    let next_seq: Option<(i64,)> = sqlx::query_as(
        "SELECT COALESCE(MAX(sequence), 0) + 1 FROM messages WHERE thread_id = $1",
    )
    .bind(thread_id)
    .fetch_optional(db)
    .await
    .ok()
    .flatten();

    let seq = next_seq.map(|(s,)| s).unwrap_or(1);

    let _ = sqlx::query(
        "INSERT INTO messages (thread_id, sequence, from_addr, content) \
         VALUES ($1, $2, $3, $4) ON CONFLICT DO NOTHING",
    )
    .bind(thread_id)
    .bind(seq)
    .bind(from_addr)
    .bind(content)
    .execute(db)
    .await;

    // 2. Optionally trigger compilation (only after agent responses)
    if !trigger_compilation {
        return;
    }
    let Some(tx) = compile_tx else { return };

    // Fetch recent messages for this thread (last 20 for compilation context)
    let rows: Vec<(String, serde_json::Value, chrono::DateTime<chrono::Utc>)> = sqlx::query_as(
        "SELECT from_addr, content, created_at FROM messages \
         WHERE thread_id = $1 ORDER BY sequence DESC LIMIT 20",
    )
    .bind(thread_id)
    .fetch_all(db)
    .await
    .unwrap_or_default();

    // Need at least 2 messages (1 human + 1 agent) to compile
    if rows.len() < 2 {
        return;
    }

    // Convert to ConversationMessage format (reversed to chronological order)
    let messages: Vec<crate::llm::ConversationMessage> = rows
        .into_iter()
        .rev()
        .map(|(addr, content, ts)| {
            let text = content
                .as_array()
                .and_then(|arr| arr.first())
                .and_then(|block| block.get("fallback").or(block.get("text")))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            crate::llm::ConversationMessage {
                from: addr,
                content: text,
                timestamp: ts.to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            }
        })
        .collect();

    // Determine user_addr: the human participant (non-agent sender)
    // Heuristic: the first sender that isn't the current from_addr (agent)
    let user_addr = messages
        .iter()
        .find(|m| m.from != from_addr)
        .map(|m| m.from.clone())
        .unwrap_or_else(|| from_addr.to_string());

    let req = crate::compiler::CompileRequest {
        user_addr,
        thread_id: thread_id.to_string(),
        messages,
    };

    if let Err(e) = tx.try_send(req) {
        tracing::warn!("[compiler] Failed to enqueue compilation: {e}");
    } else {
        tracing::info!(
            "[compiler] Triggered compilation for thread {}",
            thread_id
        );
    }
}

/// Handle compound memory JSON-RPC methods.
/// Shared by both human and agent connection handlers.
async fn handle_compound_memory(
    state: &ServerState,
    from_addr: &str,
    method: &str,
    parsed: &serde_json::Value,
    ws_sender: &Arc<Mutex<futures_util::stream::SplitSink<WebSocket, Message>>>,
) {
    let has_id = parsed.get("id").is_some();

    match method {
        "ainm.memory.graph.query" => {
            if let (Some(ref db), Some(params)) = (&state.db, parsed.get("params")) {
                let text_q = params.get("query").and_then(|q| q.as_str());
                let min_conf = params.get("min_confidence").and_then(|c| c.as_f64()).unwrap_or(0.0) as f32;
                let lim = params.get("limit").and_then(|l| l.as_i64()).unwrap_or(20);
                let temporal = params.get("temporal_filter").and_then(|t| t.as_str()).unwrap_or("current");
                let include_edges = params.get("include_edges").and_then(|e| e.as_bool()).unwrap_or(false);

                match crate::graph::query_nodes(db, from_addr, text_q, None, None, temporal, min_conf, lim).await {
                    Ok(nodes) => {
                        let node_ids: Vec<uuid::Uuid> = nodes.iter().map(|n| n.id).collect();
                        let edges = if include_edges && !node_ids.is_empty() {
                            crate::graph::get_edges_for_nodes(db, from_addr, &node_ids).await.unwrap_or_default()
                        } else {
                            vec![]
                        };
                        let total = nodes.len();
                        let result = serde_json::json!({
                            "nodes": nodes.iter().map(|n| n.to_protocol_json()).collect::<Vec<_>>(),
                            "edges": edges.iter().map(|e| e.to_protocol_json()).collect::<Vec<_>>(),
                            "total_count": total,
                            "returned_count": total,
                        });
                        if has_id {
                            let resp = protocol::new_response(parsed.get("id").cloned().unwrap(), result);
                            let mut sender = ws_sender.lock().await;
                            let _ = sender.send(Message::Text(serde_json::to_string(&resp).unwrap().into())).await;
                        }
                    }
                    Err(e) => {
                        if has_id {
                            let err = protocol::new_error(parsed.get("id").cloned().unwrap(), -32050, &format!("Graph query failed: {e}"));
                            let mut sender = ws_sender.lock().await;
                            let _ = sender.send(Message::Text(serde_json::to_string(&err).unwrap().into())).await;
                        }
                    }
                }
            } else if has_id {
                let err = protocol::new_error(parsed.get("id").cloned().unwrap(), -32062, "Compound memory not supported");
                let mut sender = ws_sender.lock().await;
                let _ = sender.send(Message::Text(serde_json::to_string(&err).unwrap().into())).await;
            }
        }
        "ainm.memory.graph.traverse" => {
            if let (Some(ref db), Some(params)) = (&state.db, parsed.get("params")) {
                let start_str = params.get("start_node").and_then(|s| s.as_str()).unwrap_or("");
                let max_depth = params.get("max_depth").and_then(|d| d.as_i64()).unwrap_or(2) as i32;
                let direction = params.get("direction").and_then(|d| d.as_str()).unwrap_or("both");

                if let Ok(start_uuid) = uuid::Uuid::parse_str(start_str) {
                    match crate::graph::traverse(db, from_addr, start_uuid, max_depth, direction, None).await {
                        Ok((nodes, edges)) => {
                            let result = serde_json::json!({
                                "subgraph": {
                                    "nodes": nodes.iter().map(|n| n.to_protocol_json()).collect::<Vec<_>>(),
                                    "edges": edges.iter().map(|e| e.to_protocol_json()).collect::<Vec<_>>(),
                                },
                                "traversal_depth_reached": max_depth,
                            });
                            if has_id {
                                let resp = protocol::new_response(parsed.get("id").cloned().unwrap(), result);
                                let mut sender = ws_sender.lock().await;
                                let _ = sender.send(Message::Text(serde_json::to_string(&resp).unwrap().into())).await;
                            }
                        }
                        Err(e) => {
                            if has_id {
                                let err = protocol::new_error(parsed.get("id").cloned().unwrap(), -32050, &format!("Traversal failed: {e}"));
                                let mut sender = ws_sender.lock().await;
                                let _ = sender.send(Message::Text(serde_json::to_string(&err).unwrap().into())).await;
                            }
                        }
                    }
                } else if has_id {
                    let err = protocol::new_error(parsed.get("id").cloned().unwrap(), -32066, "Invalid start_node UUID");
                    let mut sender = ws_sender.lock().await;
                    let _ = sender.send(Message::Text(serde_json::to_string(&err).unwrap().into())).await;
                }
            } else if has_id {
                let err = protocol::new_error(parsed.get("id").cloned().unwrap(), -32062, "Compound memory not supported");
                let mut sender = ws_sender.lock().await;
                let _ = sender.send(Message::Text(serde_json::to_string(&err).unwrap().into())).await;
            }
        }
        "ainm.memory.graph.contribute" => {
            if let (Some(ref db), Some(params)) = (&state.db, parsed.get("params")) {
                let thread_id = params.get("thread_id").and_then(|t| t.as_str()).unwrap_or("").to_string();
                if let Some(node_data) = params.get("node") {
                    let node_type = node_data.get("type").and_then(|t| t.as_str()).unwrap_or("semantic");
                    let category = node_data.get("category").and_then(|c| c.as_str()).unwrap_or("fact");
                    let label = node_data.get("label").and_then(|l| l.as_str()).unwrap_or("");
                    let confidence = node_data.get("confidence").and_then(|c| c.as_f64()).unwrap_or(0.85) as f32;
                    let confidence = confidence.min(0.85);
                    let properties = node_data.get("properties").cloned().unwrap_or(serde_json::json!({}));
                    let tags = node_data.get("tags").cloned().unwrap_or(serde_json::json!([]));
                    let provenance = serde_json::json!({
                        "thread_id": thread_id,
                        "contributed_by": from_addr,
                        "extracted_at": protocol::now_rfc3339(),
                    });

                    match crate::graph::insert_node(db, from_addr, node_type, category, label, &properties, confidence, &provenance, &tags).await {
                        Ok(node_id) => {
                            let mut edge_ids = Vec::new();
                            if let Some(edges) = params.get("edges").and_then(|e| e.as_array()) {
                                for edge_data in edges {
                                    let rel = edge_data.get("relationship").and_then(|r| r.as_str()).unwrap_or("related_to");
                                    let edge_conf = edge_data.get("confidence").and_then(|c| c.as_f64()).unwrap_or(0.85) as f32;
                                    if let Some(target_id_str) = edge_data.get("target_node").and_then(|t| t.as_str()) {
                                        if let Ok(target_uuid) = uuid::Uuid::parse_str(target_id_str) {
                                            if let Ok(eid) = crate::graph::insert_edge(db, from_addr, node_id, target_uuid, rel, 1.0, edge_conf, &provenance, &serde_json::json!({})).await {
                                                edge_ids.push(eid.to_string());
                                            }
                                        }
                                    }
                                }
                            }
                            if has_id {
                                let result = serde_json::json!({
                                    "contributed": true,
                                    "node_id": node_id.to_string(),
                                    "edge_ids": edge_ids,
                                });
                                let resp = protocol::new_response(parsed.get("id").cloned().unwrap(), result);
                                let mut sender = ws_sender.lock().await;
                                let _ = sender.send(Message::Text(serde_json::to_string(&resp).unwrap().into())).await;
                            }
                        }
                        Err(e) => {
                            if has_id {
                                let err = protocol::new_error(parsed.get("id").cloned().unwrap(), -32067, &format!("Contribution rejected: {e}"));
                                let mut sender = ws_sender.lock().await;
                                let _ = sender.send(Message::Text(serde_json::to_string(&err).unwrap().into())).await;
                            }
                        }
                    }
                }
            } else if has_id {
                let err = protocol::new_error(parsed.get("id").cloned().unwrap(), -32062, "Compound memory not supported");
                let mut sender = ws_sender.lock().await;
                let _ = sender.send(Message::Text(serde_json::to_string(&err).unwrap().into())).await;
            }
        }
        "ainm.memory.lint" => {
            if let Some(ref db) = state.db {
                let contradictions = crate::graph::find_contradictions(db, from_addr).await.unwrap_or_default();
                let orphans = crate::graph::find_orphans(db, from_addr).await.unwrap_or_default();
                let stale = crate::graph::find_stale(db, from_addr, 180).await.unwrap_or_default();
                let (total_nodes, total_edges) = crate::graph::graph_stats(db, from_addr).await.unwrap_or((0, 0));

                let mut findings = Vec::new();
                for (s, t, _) in &contradictions {
                    findings.push(serde_json::json!({"type": "contradiction", "severity": "warning", "description": "Two active nodes may assert conflicting facts", "node_ids": [s.to_string(), t.to_string()]}));
                }
                for oid in &orphans {
                    findings.push(serde_json::json!({"type": "orphan", "severity": "info", "description": "Node has no edges", "node_ids": [oid.to_string()]}));
                }
                for sid in &stale {
                    findings.push(serde_json::json!({"type": "stale", "severity": "info", "description": "Node not updated in over 6 months", "node_ids": [sid.to_string()]}));
                }

                let result = serde_json::json!({
                    "lint_id": protocol::new_message_id(),
                    "timestamp": protocol::now_rfc3339(),
                    "findings": findings,
                    "summary": {"total_nodes": total_nodes, "total_edges": total_edges, "contradictions": contradictions.len(), "orphan_nodes": orphans.len(), "stale_nodes": stale.len()},
                });
                if has_id {
                    let resp = protocol::new_response(parsed.get("id").cloned().unwrap(), result);
                    let mut sender = ws_sender.lock().await;
                    let _ = sender.send(Message::Text(serde_json::to_string(&resp).unwrap().into())).await;
                }
            } else if has_id {
                let err = protocol::new_error(parsed.get("id").cloned().unwrap(), -32062, "Compound memory not supported");
                let mut sender = ws_sender.lock().await;
                let _ = sender.send(Message::Text(serde_json::to_string(&err).unwrap().into())).await;
            }
        }
        "ainm.memory.briefing" => {
            if let (Some(ref db), Some(params)) = (&state.db, parsed.get("params")) {
                let thread_id = params.get("thread_id").and_then(|t| t.as_str()).unwrap_or("");
                let user_addr = params.get("user_identity").and_then(|u| u.as_str()).unwrap_or(from_addr);

                match crate::briefing::generate_briefing(db, user_addr, from_addr, thread_id, crate::briefing::ConsentLevel::Standard).await {
                    Ok(briefing) => {
                        if has_id {
                            let resp = protocol::new_response(parsed.get("id").cloned().unwrap(), briefing);
                            let mut sender = ws_sender.lock().await;
                            let _ = sender.send(Message::Text(serde_json::to_string(&resp).unwrap().into())).await;
                        }
                    }
                    Err(e) => {
                        if has_id {
                            let err = protocol::new_error(parsed.get("id").cloned().unwrap(), -32065, &format!("Briefing failed: {e}"));
                            let mut sender = ws_sender.lock().await;
                            let _ = sender.send(Message::Text(serde_json::to_string(&err).unwrap().into())).await;
                        }
                    }
                }
            } else if has_id {
                let err = protocol::new_error(parsed.get("id").cloned().unwrap(), -32062, "Compound memory not supported");
                let mut sender = ws_sender.lock().await;
                let _ = sender.send(Message::Text(serde_json::to_string(&err).unwrap().into())).await;
            }
        }
        "ainm.memory.briefing.ack" => {
            if let Some(params) = parsed.get("params") {
                let briefing_id = params.get("briefing_id").and_then(|b| b.as_str()).unwrap_or("unknown");
                tracing::info!("[briefing] {} acknowledged briefing {}", from_addr, briefing_id);
            }
        }
        "ainm.memory.grant" => {
            if let (Some(ref db), Some(params)) = (&state.db, parsed.get("params")) {
                let agent_addr = params.get("agent_identity").and_then(|a| a.as_str()).unwrap_or("");
                let scopes: Vec<String> = params.get("scopes").and_then(|s| s.as_array()).map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect()).unwrap_or_default();
                let consent_str = params.get("consent_level").and_then(|c| c.as_str()).unwrap_or("standard");
                let consent = crate::briefing::ConsentLevel::from_str(consent_str);

                match crate::briefing::grant_scopes(db, from_addr, agent_addr, &scopes, consent).await {
                    Ok(()) => {
                        if has_id {
                            let result = serde_json::json!({"granted": true, "agent_identity": agent_addr, "effective_scopes": scopes, "effective_consent_level": consent.as_str()});
                            let resp = protocol::new_response(parsed.get("id").cloned().unwrap(), result);
                            let mut sender = ws_sender.lock().await;
                            let _ = sender.send(Message::Text(serde_json::to_string(&resp).unwrap().into())).await;
                        }
                    }
                    Err(e) => {
                        if has_id {
                            let err = protocol::new_error(parsed.get("id").cloned().unwrap(), -32050, &format!("Grant failed: {e}"));
                            let mut sender = ws_sender.lock().await;
                            let _ = sender.send(Message::Text(serde_json::to_string(&err).unwrap().into())).await;
                        }
                    }
                }
            } else if has_id {
                let err = protocol::new_error(parsed.get("id").cloned().unwrap(), -32062, "Compound memory not supported");
                let mut sender = ws_sender.lock().await;
                let _ = sender.send(Message::Text(serde_json::to_string(&err).unwrap().into())).await;
            }
        }
        "ainm.memory.revoke" => {
            if let (Some(ref db), Some(params)) = (&state.db, parsed.get("params")) {
                let agent_addr = params.get("agent_identity").and_then(|a| a.as_str()).unwrap_or("");
                let scopes: Vec<String> = params.get("scopes").and_then(|s| s.as_array()).map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect()).unwrap_or_default();

                match crate::briefing::revoke_scopes(db, from_addr, agent_addr, &scopes).await {
                    Ok(()) => {
                        if has_id {
                            let result = serde_json::json!({"revoked": true, "agent_identity": agent_addr});
                            let resp = protocol::new_response(parsed.get("id").cloned().unwrap(), result);
                            let mut sender = ws_sender.lock().await;
                            let _ = sender.send(Message::Text(serde_json::to_string(&resp).unwrap().into())).await;
                        }
                    }
                    Err(e) => {
                        if has_id {
                            let err = protocol::new_error(parsed.get("id").cloned().unwrap(), -32050, &format!("Revoke failed: {e}"));
                            let mut sender = ws_sender.lock().await;
                            let _ = sender.send(Message::Text(serde_json::to_string(&err).unwrap().into())).await;
                        }
                    }
                }
            } else if has_id {
                let err = protocol::new_error(parsed.get("id").cloned().unwrap(), -32062, "Compound memory not supported");
                let mut sender = ws_sender.lock().await;
                let _ = sender.send(Message::Text(serde_json::to_string(&err).unwrap().into())).await;
            }
        }
        "ainm.memory.export" => {
            if let Some(ref db) = state.db {
                let include_superseded = parsed.get("params").and_then(|p| p.get("include_superseded")).and_then(|v| v.as_bool()).unwrap_or(false);

                match crate::graph::export_graph(db, from_addr, include_superseded).await {
                    Ok((nodes, edges)) => {
                        let earliest = nodes.first().map(|n| n.valid_from.to_rfc3339_opts(chrono::SecondsFormat::Millis, true));
                        let latest = nodes.last().map(|n| n.valid_from.to_rfc3339_opts(chrono::SecondsFormat::Millis, true));
                        let result = serde_json::json!({
                            "format": "ainm-memory-v1",
                            "version": "1.0.0",
                            "exported_at": protocol::now_rfc3339(),
                            "user_identity": from_addr,
                            "graph": {
                                "nodes": nodes.iter().map(|n| n.to_protocol_json()).collect::<Vec<_>>(),
                                "edges": edges.iter().map(|e| e.to_protocol_json()).collect::<Vec<_>>(),
                            },
                            "metadata": {"node_count": nodes.len(), "edge_count": edges.len(), "earliest_node": earliest, "latest_node": latest}
                        });
                        if has_id {
                            let resp = protocol::new_response(parsed.get("id").cloned().unwrap(), result);
                            let mut sender = ws_sender.lock().await;
                            let _ = sender.send(Message::Text(serde_json::to_string(&resp).unwrap().into())).await;
                        }
                    }
                    Err(e) => {
                        if has_id {
                            let err = protocol::new_error(parsed.get("id").cloned().unwrap(), -32050, &format!("Export failed: {e}"));
                            let mut sender = ws_sender.lock().await;
                            let _ = sender.send(Message::Text(serde_json::to_string(&err).unwrap().into())).await;
                        }
                    }
                }
            } else if has_id {
                let err = protocol::new_error(parsed.get("id").cloned().unwrap(), -32062, "Compound memory not supported");
                let mut sender = ws_sender.lock().await;
                let _ = sender.send(Message::Text(serde_json::to_string(&err).unwrap().into())).await;
            }
        }
        _ => {
            if has_id {
                let err = protocol::new_error(parsed.get("id").cloned().unwrap(), -32601, "Method not found");
                let mut sender = ws_sender.lock().await;
                let _ = sender.send(Message::Text(serde_json::to_string(&err).unwrap().into())).await;
            }
        }
    }
}

/// Handle the agent's WebSocket connection.
async fn handle_agent_connection(
    ws_sender: Arc<Mutex<futures_util::stream::SplitSink<WebSocket, Message>>>,
    mut ws_receiver: futures_util::stream::SplitStream<WebSocket>,
    state: ServerState,
    from_addr: String,
    init_req: &serde_json::Value,
) {
    // Build a minimal agent card from the init request capabilities
    let capabilities = init_req
        .get("params")
        .and_then(|p| p.get("capabilities"))
        .cloned()
        .unwrap_or(serde_json::json!({}));
    let agent_card = serde_json::json!({
        "identity": from_addr,
        "capabilities": capabilities,
        "connected_at": protocol::now_rfc3339(),
    });

    // Register this agent in the AgentRegistry
    let _from_registry_rx = state.agent_registry.register(&from_addr, &agent_card).await;
    tracing::info!("Agent registered: {}", from_addr);

    let mut from_human_rx = state.to_agent_tx.subscribe();
    let human_registry = state.human_registry.clone();
    let identity_store = state.identity_store.clone();
    let archival_store = state.archival_store.clone();
    let ws_sender_clone = ws_sender.clone();

    // Task: forward human messages to agent WebSocket
    let forward_handle = tokio::spawn(async move {
        while let Ok(msg) = from_human_rx.recv().await {
            let mut sender = ws_sender_clone.lock().await;
            if sender.send(Message::Text(msg.into())).await.is_err() {
                break;
            }
        }
    });

    // Track stream_id -> thread_id for federation relay of delta/end events
    let mut stream_thread_map: std::collections::HashMap<String, String> = std::collections::HashMap::new();

    // Main loop: receive agent messages and forward to human, or drain on shutdown.
    let mut shutdown_rx = state.shutdown.subscribe();
    if !state.shutdown.is_shutting_down() {
        loop {
            tokio::select! {
                biased;
                _ = shutdown_rx.changed() => {
                    tracing::info!("[lifecycle] draining agent connection {from_addr}");
                    break;
                }
                maybe = ws_receiver.next() => {
                    let Some(Ok(msg)) = maybe else { break; };
                    if let Message::Text(text) = msg {
            let text_str = text.to_string();
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&text_str) {
                let method = parsed
                    .get("method")
                    .and_then(|m| m.as_str())
                    .unwrap_or("");
                let has_id = parsed.get("id").is_some();

                match method {
                    "ainm.message.send" => {
                        if let Some(params) = parsed.get("params") {
                            let from = params
                                .get("from")
                                .and_then(|f| f.as_str())
                                .unwrap_or("");
                            let thread_id = params
                                .get("thread_id")
                                .and_then(|t| t.as_str())
                                .unwrap_or("");
                            let verified = if let Some(pk) =
                                identity_store.get_public_key_bytes(from)
                            {
                                signing::verify_signature(&pk, params).unwrap_or(false)
                            } else {
                                false
                            };

                            // Federation-aware routing for agent responses
                            if let Some(ref fed) = state.federation {
                                let origin = federation::parse_origin_domain(thread_id)
                                    .unwrap_or("");

                                if origin != fed.server_id && !origin.is_empty() {
                                    // Thread originated on remote server: relay response back
                                    fed.relay_sender.enqueue(origin, params.clone());
                                } else {
                                    // Local thread: deliver to thread participants
                                    let deliver = protocol::new_notification(
                                        "ainm.message.deliver",
                                        serde_json::json!({
                                            "message": params,
                                            "verified": verified,
                                        }),
                                    );
                                    let deliver_text = serde_json::to_string(&deliver).unwrap();
                                    human_registry.send_to_thread(thread_id, &deliver_text).await;
                                }
                            } else {
                                // Legacy mode
                                let deliver = protocol::new_notification(
                                    "ainm.message.deliver",
                                    serde_json::json!({
                                        "message": params,
                                        "verified": verified,
                                    }),
                                );
                                let deliver_text = serde_json::to_string(&deliver).unwrap();
                                human_registry.send_to_thread(thread_id, &deliver_text).await;
                            }

                            if has_id {
                                let resp = protocol::new_response(
                                    parsed.get("id").cloned().unwrap(),
                                    serde_json::json!({
                                        "accepted": true,
                                        "server_timestamp": protocol::now_rfc3339()
                                    }),
                                );
                                let mut sender = ws_sender.lock().await;
                                let _ = sender
                                    .send(Message::Text(
                                        serde_json::to_string(&resp).unwrap().into(),
                                    ))
                                    .await;
                            }

                            // Store agent message and trigger compilation
                            // Agent responses complete an exchange — richest knowledge source
                            if let Some(ref db) = state.db {
                                let thread_id = params
                                    .get("thread_id")
                                    .and_then(|t| t.as_str())
                                    .unwrap_or("");
                                let content = params
                                    .get("content")
                                    .cloned()
                                    .unwrap_or(serde_json::json!([]));
                                store_and_maybe_compile(
                                    db,
                                    &state.compile_tx,
                                    thread_id,
                                    from,
                                    &content,
                                    true, // trigger compilation on agent responses
                                )
                                .await;
                            }
                        }
                    }
                    "ainm.memory.store" => {
                        if let Some(params) = parsed.get("params") {
                            let tier = params
                                .get("tier")
                                .and_then(|t| t.as_str())
                                .unwrap_or("");
                            let thread_id = params
                                .get("thread_id")
                                .and_then(|t| t.as_str())
                                .unwrap_or("")
                                .to_string();

                            if tier != "archival" {
                                // Error -32051: unsupported tier
                                if has_id {
                                    let err = protocol::new_error(
                                        parsed.get("id").cloned().unwrap(),
                                        -32051,
                                        "Only archival tier is supported",
                                    );
                                    let mut sender = ws_sender.lock().await;
                                    let _ = sender
                                        .send(Message::Text(
                                            serde_json::to_string(&err).unwrap().into(),
                                        ))
                                        .await;
                                }
                            } else if let Some(fact) = params.get("fact") {
                                let key = fact
                                    .get("key")
                                    .and_then(|k| k.as_str())
                                    .unwrap_or("unknown")
                                    .to_string();
                                let value = fact.get("value").cloned().unwrap_or(serde_json::json!(""));
                                let tags = fact
                                    .get("tags")
                                    .cloned()
                                    .unwrap_or(serde_json::json!([]));
                                let fact_id = protocol::new_message_id();
                                let stored_at = protocol::now_rfc3339();
                                let from_identity = from_addr.clone();

                                let archival_fact = serde_json::json!({
                                    "fact_id": fact_id,
                                    "key": key,
                                    "value": value,
                                    "tags": tags,
                                    "stored_by": from_identity,
                                    "stored_at": stored_at
                                });

                                // Store: PostgreSQL if available, else in-memory fallback
                                if let Some(ref db) = state.db {
                                    if let Err(e) = crate::repository::store_fact(
                                        db,
                                        &fact_id,
                                        &from_identity,
                                        Some(&thread_id),
                                        &key,
                                        &value,
                                        &tags,
                                        &from_identity,
                                    ).await {
                                        tracing::error!("[memory] PostgreSQL store failed: {e}, falling back to in-memory");
                                        archival_store.lock().await.insert(key.clone(), archival_fact.clone());
                                    }
                                } else {
                                    archival_store.lock().await.insert(key.clone(), archival_fact.clone());
                                }

                                // Send response
                                if has_id {
                                    let resp = protocol::new_response(
                                        parsed.get("id").cloned().unwrap(),
                                        archival_fact.clone(),
                                    );
                                    let mut sender = ws_sender.lock().await;
                                    let _ = sender
                                        .send(Message::Text(
                                            serde_json::to_string(&resp).unwrap().into(),
                                        ))
                                        .await;
                                }

                                // Forward the store result as a notification to human for rendering
                                let store_notify = protocol::new_notification(
                                    "ainm.memory.stored",
                                    serde_json::json!({
                                        "thread_id": thread_id,
                                        "key": key,
                                        "status": "stored"
                                    }),
                                );
                                human_registry.send_to_thread(&thread_id, &serde_json::to_string(&store_notify).unwrap()).await;

                                // After successful memory store in a federated thread,
                                // push context update to remote servers (ainm.context-push)
                                if let Some(ref fed) = state.federation {
                                    let origin = federation::parse_origin_domain(&thread_id).unwrap_or("");
                                    if origin == fed.server_id {
                                        // We are origin -- push the stored fact as a context frame to remotes
                                        let context_frame = serde_json::json!({
                                            "frame_type": "active_context",
                                            "thread_id": thread_id,
                                            "key": key,
                                            "data": archival_fact,
                                            "timestamp": protocol::now_rfc3339()
                                        });
                                        if let Some((_, remotes)) = fed.thread_registry.get_info(&thread_id) {
                                            for remote in &remotes {
                                                if remote != &fed.server_id {
                                                    let push_payload = serde_json::json!({
                                                        "jsonrpc": "2.0",
                                                        "method": "ainm.context-push",
                                                        "id": protocol::new_message_id(),
                                                        "params": {
                                                            "thread_id": thread_id,
                                                            "context_frames": [context_frame]
                                                        }
                                                    });
                                                    fed.relay_sender.enqueue(remote, push_payload);
                                                    tracing::info!("[federation] Pushed context frame to {}", remote);
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    "ainm.memory.query" => {
                        if let Some(params) = parsed.get("params") {
                            let thread_id = params
                                .get("thread_id")
                                .and_then(|t| t.as_str())
                                .unwrap_or("")
                                .to_string();
                            let key_prefix = params
                                .get("key")
                                .and_then(|k| k.as_str())
                                .unwrap_or("")
                                .to_string();
                            let limit = params
                                .get("limit")
                                .and_then(|l| l.as_u64())
                                .unwrap_or(10) as usize;

                            // Query: PostgreSQL if available, else in-memory fallback
                            let results: Vec<serde_json::Value> = if let Some(ref db) = state.db {
                                match crate::repository::query_facts(
                                    db,
                                    &from_addr,
                                    if key_prefix.is_empty() { None } else { Some(&key_prefix) },
                                    limit as i64,
                                ).await {
                                    Ok(rows) => rows.iter().map(|r| r.to_protocol_json()).collect(),
                                    Err(e) => {
                                        tracing::error!("[memory] PostgreSQL query failed: {e}, falling back to in-memory");
                                        let store = archival_store.lock().await;
                                        store.iter()
                                            .filter(|(k, _)| key_prefix.is_empty() || k.starts_with(&key_prefix))
                                            .take(limit)
                                            .map(|(_, v)| v.clone())
                                            .collect()
                                    }
                                }
                            } else {
                                let store = archival_store.lock().await;
                                store.iter()
                                    .filter(|(k, _)| key_prefix.is_empty() || k.starts_with(&key_prefix))
                                    .take(limit)
                                    .map(|(_, v)| v.clone())
                                    .collect()
                            };

                            let query_result = serde_json::json!({
                                "results": results,
                                "truncated": false
                            });

                            // Send query response
                            if has_id {
                                let resp = protocol::new_response(
                                    parsed.get("id").cloned().unwrap(),
                                    query_result.clone(),
                                );
                                let mut sender = ws_sender.lock().await;
                                let _ = sender
                                    .send(Message::Text(
                                        serde_json::to_string(&resp).unwrap().into(),
                                    ))
                                    .await;
                            }

                            // Forward query results to human for rendering
                            let query_notify = protocol::new_notification(
                                "ainm.memory.query.result",
                                query_result,
                            );
                            human_registry.send_to_thread(&thread_id, &serde_json::to_string(&query_notify).unwrap()).await;

                            // Build and send ainm.context.frame notification
                            let archival_facts: Vec<serde_json::Value> = if let Some(ref db) = state.db {
                                match crate::repository::get_all_facts(db, &from_addr).await {
                                    Ok(rows) => rows.iter().map(|r| r.to_protocol_json()).collect(),
                                    Err(_) => {
                                        let store = archival_store.lock().await;
                                        store.values().cloned().collect()
                                    }
                                }
                            } else {
                                let store = archival_store.lock().await;
                                store.values().cloned().collect()
                            };
                            let now = protocol::now_rfc3339();
                            let msg_id = protocol::new_message_id();

                            let context_frame = protocol::new_notification(
                                "ainm.context.frame",
                                serde_json::json!({
                                    "thread_id": thread_id,
                                    "frame": {
                                        "active_context": {
                                            "recent_messages": [
                                                {
                                                    "id": msg_id,
                                                    "from": "alice@ainm.local",
                                                    "timestamp": now,
                                                    "summary": "User requested memory operations"
                                                }
                                            ],
                                            "window_start": now,
                                            "message_count": 1
                                        },
                                        "conversation_summary": "Thread exercising memory operations.",
                                        "archival_facts": archival_facts,
                                        "participants": [
                                            { "identity": "alice@ainm.local", "type": "human", "presence": "online" },
                                            { "identity": "helper@ainm.local", "type": "agent", "presence": "available" }
                                        ]
                                    }
                                }),
                            );
                            human_registry.send_to_thread(&thread_id, &serde_json::to_string(&context_frame).unwrap()).await;
                        }
                    }
                    "ainm.memory.evict" => {
                        if let Some(params) = parsed.get("params") {
                            let tier = params
                                .get("tier")
                                .and_then(|t| t.as_str())
                                .unwrap_or("");
                            let thread_id = params
                                .get("thread_id")
                                .and_then(|t| t.as_str())
                                .unwrap_or("")
                                .to_string();

                            if tier != "archival" {
                                if has_id {
                                    let err = protocol::new_error(
                                        parsed.get("id").cloned().unwrap(),
                                        -32051,
                                        "Only archival tier supports eviction",
                                    );
                                    let mut sender = ws_sender.lock().await;
                                    let _ = sender
                                        .send(Message::Text(
                                            serde_json::to_string(&err).unwrap().into(),
                                        ))
                                        .await;
                                }
                            } else {
                                let key = params
                                    .get("key")
                                    .and_then(|k| k.as_str())
                                    .unwrap_or("")
                                    .to_string();

                                if key.is_empty() {
                                    if has_id {
                                        let err = protocol::new_error(
                                            parsed.get("id").cloned().unwrap(),
                                            -32053,
                                            "Missing required parameter: key",
                                        );
                                        let mut sender = ws_sender.lock().await;
                                        let _ = sender
                                            .send(Message::Text(
                                                serde_json::to_string(&err).unwrap().into(),
                                            ))
                                            .await;
                                    }
                                } else {
                                    // Evict: PostgreSQL if available, else in-memory fallback
                                    let evicted = if let Some(ref db) = state.db {
                                        match crate::repository::evict_fact(db, &from_addr, &key).await {
                                            Ok(deleted) => deleted,
                                            Err(e) => {
                                                tracing::error!("[memory] PostgreSQL evict failed: {e}, falling back to in-memory");
                                                archival_store.lock().await.remove(&key).is_some()
                                            }
                                        }
                                    } else {
                                        archival_store.lock().await.remove(&key).is_some()
                                    };

                                    if evicted {
                                        if has_id {
                                            let resp = protocol::new_response(
                                                parsed.get("id").cloned().unwrap(),
                                                serde_json::json!({ "evicted": true }),
                                            );
                                            let mut sender = ws_sender.lock().await;
                                            let _ = sender
                                                .send(Message::Text(
                                                    serde_json::to_string(&resp).unwrap().into(),
                                                ))
                                                .await;
                                        }
                                        let evict_notify = protocol::new_notification(
                                            "ainm.memory.evicted",
                                            serde_json::json!({
                                                "thread_id": thread_id,
                                                "key": key,
                                                "status": "evicted"
                                            }),
                                        );
                                        human_registry.send_to_thread(&thread_id, &serde_json::to_string(&evict_notify).unwrap()).await;
                                    } else if has_id {
                                        let err = protocol::new_error(
                                            parsed.get("id").cloned().unwrap(),
                                            -32054,
                                            "Fact not found",
                                        );
                                        let mut sender = ws_sender.lock().await;
                                        let _ = sender
                                            .send(Message::Text(
                                                serde_json::to_string(&err).unwrap().into(),
                                            ))
                                            .await;
                                    }
                                }
                            }
                        }
                    }
                    // ── Compound Memory: delegated to shared handler ──
                    m if m.starts_with("ainm.memory.") => {
                        handle_compound_memory(&state, &from_addr, m, &parsed, &ws_sender).await;
                    }
                    "ainm.stream.start"
                    | "ainm.stream.delta"
                    | "ainm.stream.end"
                    | "ainm.presence.update" => {
                        // Deliver to thread participants
                        let stream_tid = parsed.get("params")
                            .and_then(|p| p.get("thread_id"))
                            .and_then(|t| t.as_str())
                            .or_else(|| {
                                // For delta/end events, look up thread_id from stream_id mapping
                                parsed.get("params")
                                    .and_then(|p| p.get("stream_id"))
                                    .and_then(|s| s.as_str())
                                    .and_then(|sid| stream_thread_map.get(sid))
                                    .map(|s| s.as_str())
                            })
                            .unwrap_or("");
                        human_registry.send_to_thread(stream_tid, &text_str).await;

                        // Federation relay if thread is federated (D-04: stream events via ainm.relay)
                        if let Some(ref fed) = state.federation {
                            let thread_id_opt = parsed.get("params")
                                .and_then(|p| p.get("thread_id"))
                                .and_then(|t| t.as_str());

                            let effective_thread_id = if let Some(tid) = thread_id_opt {
                                // StreamStart — track the stream_id -> thread_id mapping
                                if method == "ainm.stream.start" {
                                    if let Some(sid) = parsed.get("params").and_then(|p| p.get("stream_id")).and_then(|s| s.as_str()) {
                                        stream_thread_map.insert(sid.to_string(), tid.to_string());
                                    }
                                }
                                Some(tid.to_string())
                            } else {
                                // Delta/End — look up thread_id from stream_id
                                parsed.get("params")
                                    .and_then(|p| p.get("stream_id"))
                                    .and_then(|s| s.as_str())
                                    .and_then(|sid| stream_thread_map.get(sid).cloned())
                            };

                            if let Some(tid) = effective_thread_id {
                                let origin = federation::parse_origin_domain(&tid).unwrap_or("");
                                if !origin.is_empty() && origin != fed.server_id {
                                    // Relay to origin server (which will fan out)
                                    fed.relay_sender.enqueue(origin, parsed.clone());
                                } else if origin == fed.server_id {
                                    // We ARE origin — fan out to all participating remote servers
                                    if let Some((_, remotes)) = fed.thread_registry.get_info(&tid) {
                                        for remote in &remotes {
                                            if remote != &fed.server_id {
                                                fed.relay_sender.enqueue(remote, parsed.clone());
                                            }
                                        }
                                    }
                                }
                            }

                            // Clean up stream_thread_map on StreamEnd
                            if method == "ainm.stream.end" {
                                if let Some(sid) = parsed.get("params").and_then(|p| p.get("stream_id")).and_then(|s| s.as_str()) {
                                    stream_thread_map.remove(sid);
                                }
                            }
                        }
                    }
                    _ => {
                        tracing::debug!("Agent sent unknown method: {}", method);
                    }
                }
            }
                    } // end if let Message::Text
                } // end maybe arm
            } // end select!
        } // end loop
    } // end if !is_shutting_down
    // On break, close the socket cleanly so the client sees a Close frame.
    {
        let mut sender = ws_sender.lock().await;
        let _ = sender.send(Message::Close(None)).await;
    }

    forward_handle.abort();

    // Deregister this agent from the AgentRegistry
    state.agent_registry.cleanup(&from_addr).await;
    tracing::info!("Agent deregistered: {}", from_addr);
}

/// Handle the human's WebSocket connection.
async fn handle_human_connection(
    ws_sender: Arc<Mutex<futures_util::stream::SplitSink<WebSocket, Message>>>,
    mut ws_receiver: futures_util::stream::SplitStream<WebSocket>,
    state: ServerState,
    human_addr: String,
) {
    // Register this human connection in the registry
    let mut from_registry_rx = state.human_registry.register(&human_addr).await;
    let registry_for_cleanup = state.human_registry.clone();
    let cleanup_addr = human_addr.clone();

    let to_agent_tx = state.to_agent_tx.clone();
    let identity_store = state.identity_store.clone();
    let threads = state.threads.clone();
    let archival_store = state.archival_store.clone();
    let ws_sender_clone = ws_sender.clone();

    // Task: forward compilation notifications scoped to this user
    let ws_sender_compile = ws_sender.clone();
    let user_identity = human_addr.clone();
    let compile_forward_handle = if let Some(ref notify_tx) = state.compile_notify_tx {
        let mut rx = notify_tx.subscribe();
        Some(tokio::spawn(async move {
            while let Ok(msg) = rx.recv().await {
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&msg) {
                    let target = parsed
                        .get("params")
                        .and_then(|p| p.get("user_identity"))
                        .and_then(|u| u.as_str())
                        .unwrap_or("");
                    if target != user_identity {
                        continue;
                    }
                }
                let mut sender = ws_sender_compile.lock().await;
                if sender.send(Message::Text(msg.into())).await.is_err() {
                    break;
                }
            }
        }))
    } else {
        None
    };

    // Task: forward registry messages (from agent handler) to this human's WebSocket
    let forward_handle = tokio::spawn(async move {
        while let Some(msg) = from_registry_rx.recv().await {
            let mut sender = ws_sender_clone.lock().await;
            if sender.send(Message::Text(msg.into())).await.is_err() {
                break;
            }
        }
    });

    // Main loop: receive human messages, or drain on shutdown.
    let mut shutdown_rx = state.shutdown.subscribe();
    if !state.shutdown.is_shutting_down() {
        loop {
            tokio::select! {
                biased;
                _ = shutdown_rx.changed() => {
                    tracing::info!("[lifecycle] draining human connection {human_addr}");
                    break;
                }
                maybe = ws_receiver.next() => {
                    let Some(Ok(msg)) = maybe else { break; };
                    if let Message::Text(text) = msg {
            let text_str = text.to_string();
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&text_str) {
                let method = parsed
                    .get("method")
                    .and_then(|m| m.as_str())
                    .unwrap_or("");
                let req_id = parsed.get("id").cloned().unwrap_or(serde_json::json!(null));

                match method {
                    "ainm.thread.create" => {
                        // In federation mode, use ThreadRegistry to create thread
                        // with origin domain embedded per D-12
                        let thread_id = if let Some(ref fed) = state.federation {
                            let tid = fed.thread_registry.create_thread(&fed.server_id);
                            // Pre-populate thread with all known remote servers
                            // so the first human message gets relayed immediately
                            for remote_id in fed.remote_cache.keys() {
                                if remote_id != fed.server_id {
                                    fed.thread_registry.add_server(&tid, &remote_id);
                                }
                            }
                            tid
                        } else {
                            parsed
                                .get("params")
                                .and_then(|p| p.get("thread_id"))
                                .and_then(|t| t.as_str())
                                .unwrap_or("")
                                .to_string()
                        };
                        threads.lock().await.push(thread_id.clone());
                        state.human_registry.join_thread(&human_addr, &thread_id).await;

                        // After thread creation with federation, push existing context
                        // frames to remote servers so they start with knowledge
                        if let Some(ref fed) = state.federation {
                            let store = archival_store.lock().await;
                            if !store.is_empty() {
                                let context_frames: Vec<serde_json::Value> = store
                                    .iter()
                                    .map(|(key, value)| {
                                        serde_json::json!({
                                            "frame_type": "active_context",
                                            "thread_id": thread_id,
                                            "key": key,
                                            "data": value,
                                            "timestamp": protocol::now_rfc3339()
                                        })
                                    })
                                    .collect();

                                // Push to all remote participating servers
                                if let Some((_, remotes)) = fed.thread_registry.get_info(&thread_id) {
                                    for remote in &remotes {
                                        if remote != &fed.server_id {
                                            let push_payload = serde_json::json!({
                                                "jsonrpc": "2.0",
                                                "method": "ainm.context-push",
                                                "id": protocol::new_message_id(),
                                                "params": {
                                                    "thread_id": thread_id,
                                                    "context_frames": context_frames
                                                }
                                            });
                                            fed.relay_sender.enqueue(remote, push_payload);
                                            tracing::info!(
                                                "[federation] Pushed {} context frames to {}",
                                                context_frames.len(),
                                                remote
                                            );
                                        }
                                    }
                                }
                            }
                        }

                        let resp = protocol::new_response(
                            req_id,
                            serde_json::json!({
                                "thread_id": thread_id,
                                "created": true
                            }),
                        );
                        let mut sender = ws_sender.lock().await;
                        let _ = sender
                            .send(Message::Text(
                                serde_json::to_string(&resp).unwrap().into(),
                            ))
                            .await;
                    }
                    "ainm.message.send" => {
                        if let Some(params) = parsed.get("params") {
                            let from = params
                                .get("from")
                                .and_then(|f| f.as_str())
                                .unwrap_or("");
                            let verified = if let Some(pk) =
                                identity_store.get_public_key_bytes(from)
                            {
                                signing::verify_signature(&pk, params).unwrap_or(false)
                            } else {
                                false
                            };

                            // Track thread participation for scoped delivery
                            let msg_thread_id = params
                                .get("thread_id")
                                .and_then(|t| t.as_str())
                                .unwrap_or("");
                            if !msg_thread_id.is_empty() {
                                state.human_registry.join_thread(&human_addr, msg_thread_id).await;
                            }

                            // Federation-aware routing
                            if let Some(ref fed) = state.federation {
                                let thread_id = params
                                    .get("thread_id")
                                    .and_then(|t| t.as_str())
                                    .unwrap_or("");
                                let origin = federation::parse_origin_domain(thread_id)
                                    .unwrap_or("");

                                if origin == fed.server_id || origin.is_empty() {
                                    // We are origin: deliver locally AND relay to remote servers
                                    let deliver = protocol::new_notification(
                                        "ainm.message.deliver",
                                        serde_json::json!({
                                            "message": params,
                                            "verified": verified,
                                        }),
                                    );
                                    let deliver_text = serde_json::to_string(&deliver).unwrap();
                                    // Deliver locally (to agent if connected via WS)
                                    match to_agent_tx.send(deliver_text.clone()) {
                                        Ok(n) => tracing::info!("[routing] Delivered to {n} agent subscriber(s) (federated)"),
                                        Err(e) => tracing::warn!("[routing] No agent subscribers (federated): {e}"),
                                    }
                                    // Also deliver via federation local_delivery_tx
                                    let _ = fed.local_delivery_tx.send(deliver_text);

                                    // Relay to all remote participating servers
                                    if let Some((_, servers)) = fed.thread_registry.get_info(thread_id) {
                                        for server in servers {
                                            fed.relay_sender.enqueue(&server, params.clone());
                                        }
                                    }

                                    // Relay to known remotes not yet in the thread registry
                                    let remote_domains: Vec<String> = {
                                        let registered: std::collections::HashSet<String> = fed
                                            .thread_registry
                                            .get_info(thread_id)
                                            .map(|(_, servers)| servers)
                                            .unwrap_or_default();
                                        fed.remote_cache
                                            .keys()
                                            .into_iter()
                                            .filter(|id| id != &fed.server_id && !registered.contains(id))
                                            .collect()
                                    };
                                    for domain in remote_domains {
                                        fed.thread_registry.add_server(thread_id, &domain);
                                        fed.relay_sender.enqueue(&domain, params.clone());
                                    }
                                } else {
                                    // Thread originated on remote server: relay to origin
                                    fed.relay_sender.enqueue(origin, params.clone());
                                }
                            } else {
                                // Legacy mode: deliver locally
                                let deliver = protocol::new_notification(
                                    "ainm.message.deliver",
                                    serde_json::json!({
                                        "message": params,
                                        "verified": verified,
                                    }),
                                );
                                let deliver_text = serde_json::to_string(&deliver).unwrap();
                                match to_agent_tx.send(deliver_text) {
                                    Ok(n) => tracing::info!("[routing] Delivered to {n} agent subscriber(s)"),
                                    Err(e) => tracing::warn!("[routing] No agent subscribers: {e}"),
                                }
                            }

                            let resp = protocol::new_response(
                                req_id,
                                serde_json::json!({
                                    "accepted": true,
                                    "server_timestamp": protocol::now_rfc3339()
                                }),
                            );
                            let mut sender = ws_sender.lock().await;
                            let _ = sender
                                .send(Message::Text(
                                    serde_json::to_string(&resp).unwrap().into(),
                                ))
                                .await;

                            // Store human message (no compilation trigger — wait for agent response)
                            if let Some(ref db) = state.db {
                                let thread_id = params
                                    .get("thread_id")
                                    .and_then(|t| t.as_str())
                                    .unwrap_or("");
                                let content = params
                                    .get("content")
                                    .cloned()
                                    .unwrap_or(serde_json::json!([]));
                                store_and_maybe_compile(
                                    db,
                                    &state.compile_tx,
                                    thread_id,
                                    from,
                                    &content,
                                    false, // don't trigger compilation on human messages
                                )
                                .await;
                            }

                            // Hosted agent: if provisioned + enabled and this thread/message
                            // targets it, run the loop in the background (streams its reply
                            // back to thread participants).
                            if let Some(agent) = state.hosted_agent.clone() {
                                if agent.enabled {
                                    let content = params
                                        .get("content")
                                        .cloned()
                                        .unwrap_or(serde_json::json!([]));
                                    let targets_agent = msg_thread_id.contains("assistant@")
                                        || params.get("to").and_then(|t| t.as_str())
                                            .map(|t| t == agent.address).unwrap_or(false)
                                        || content_mentions(&content, &agent.address);
                                    tracing::debug!(
                                        "[agent-trigger] thread={msg_thread_id} to={:?} agent={} targets_agent={targets_agent}",
                                        params.get("to").and_then(|t| t.as_str()),
                                        agent.address,
                                    );
                                    if targets_agent {
                                        let state2 = state.clone();
                                        let tid = msg_thread_id.to_string();
                                        let requester = from.to_string();
                                        let user_text = content_to_plain(&content);
                                        state.shutdown.spawn_turn(async move {
                                            crate::runtime_server::handle_turn(&state2, &tid, &requester, &user_text).await;
                                        });
                                    }
                                }
                            }
                        }
                    }
                    "ainm.stream.cancel" => {
                        let _ = to_agent_tx.send(text_str);
                    }
                    m if m.starts_with("ainm.memory.") => {
                        // Compound memory operations — shared with agent handler
                        handle_compound_memory(
                            &state, &human_addr, m, &parsed, &ws_sender,
                        ).await;
                    }
                    _ => {
                        let err = protocol::new_error(req_id, -32601, "Method not found");
                        let mut sender = ws_sender.lock().await;
                        let _ = sender
                            .send(Message::Text(
                                serde_json::to_string(&err).unwrap().into(),
                            ))
                            .await;
                    }
                }
            }
                    } // end if let Message::Text
                } // end maybe arm
            } // end select!
        } // end loop
    } // end if !is_shutting_down
    // On break, close the socket cleanly so the client sees a Close frame.
    {
        let mut sender = ws_sender.lock().await;
        let _ = sender.send(Message::Close(None)).await;
    }

    forward_handle.abort();
    if let Some(h) = compile_forward_handle {
        h.abort();
    }
    // Deregister this connection from the human registry
    registry_for_cleanup.cleanup(&cleanup_addr).await;
}
