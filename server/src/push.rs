//! APNs push notifications (M4 — background message delivery).
//!
//! Env-gated like telemetry: all four `AINM_APNS__*` vars present → pushes go
//! out; any missing → `PushSender::Noop` and the server behaves exactly as
//! before. The provider token (ES256 JWT over the Apple-issued .p8 key) and
//! the payload are pure functions, unit-tested without Apple credentials; the
//! live HTTP/2 delivery is verified on a physical device at submission time.

use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use base64::Engine as _;
use sqlx::PgPool;

// --- Settings ---

/// APNs provider credentials + topic. Built from env by `settings_with`.
pub struct ApnsSettings {
    /// Path to the Apple-issued `.p8` PKCS#8 PEM (AuthKey_<KEY_ID>.p8).
    pub key_path: String,
    pub key_id: String,
    pub team_id: String,
    /// The app's bundle id (`apns-topic` header).
    pub topic: String,
    pub sandbox: bool,
}

impl ApnsSettings {
    pub fn host(&self) -> &'static str {
        if self.sandbox {
            "https://api.sandbox.push.apple.com"
        } else {
            "https://api.push.apple.com"
        }
    }
}

/// Pure env core: `Some(settings)` only when all four required vars are set.
pub fn settings_with(get: &dyn Fn(&str) -> Option<String>) -> Option<ApnsSettings> {
    let key_path = get("AINM_APNS__KEY_PATH")?;
    let key_id = get("AINM_APNS__KEY_ID")?;
    let team_id = get("AINM_APNS__TEAM_ID")?;
    let topic = get("AINM_APNS__TOPIC")?;
    let sandbox = matches!(
        get("AINM_APNS__SANDBOX").as_deref(),
        Some("1") | Some("true") | Some("yes")
    );
    Some(ApnsSettings {
        key_path,
        key_id,
        team_id,
        topic,
        sandbox,
    })
}

/// Env-backed wrapper.
pub fn settings_from_env() -> Option<ApnsSettings> {
    settings_with(&|k| std::env::var(k).ok())
}

// --- Payload ---

/// Maximum preview length shown in a notification body.
const PREVIEW_CHARS: usize = 140;

/// The APNs JSON payload: standard `aps` alert plus a custom `thread_id` the
/// client reads on tap to deep-link into the conversation. `aps.thread-id`
/// additionally groups notifications per conversation in Notification Center.
pub fn apns_payload(sender_name: &str, preview: &str, thread_id: &str) -> serde_json::Value {
    let body: String = preview.chars().take(PREVIEW_CHARS).collect();
    serde_json::json!({
        "aps": {
            "alert": { "title": sender_name, "body": body },
            "sound": "default",
            "thread-id": thread_id,
        },
        "thread_id": thread_id,
    })
}

// --- Provider token (ES256 JWT) ---

/// Build the APNs provider-authentication JWT: `ES256`, `kid` header, `iss` =
/// team id, `iat` = issue time. `p8_pem` is the Apple `.p8` file's contents.
pub fn provider_jwt(p8_pem: &str, key_id: &str, team_id: &str, iat: i64) -> Result<String, String> {
    // Extract the base64 body between the PEM guards.
    let body: String = p8_pem
        .lines()
        .filter(|l| !l.starts_with("-----"))
        .collect::<Vec<_>>()
        .join("");
    let pkcs8 = STANDARD
        .decode(body.trim())
        .map_err(|e| format!("invalid .p8 PEM body: {e}"))?;

    let rng = ring::rand::SystemRandom::new();
    let key_pair = ring::signature::EcdsaKeyPair::from_pkcs8(
        &ring::signature::ECDSA_P256_SHA256_FIXED_SIGNING,
        &pkcs8,
        &rng,
    )
    .map_err(|e| format!("invalid .p8 key: {e}"))?;

    let header = URL_SAFE_NO_PAD.encode(
        serde_json::json!({ "alg": "ES256", "kid": key_id })
            .to_string()
            .as_bytes(),
    );
    let claims = URL_SAFE_NO_PAD.encode(
        serde_json::json!({ "iss": team_id, "iat": iat })
            .to_string()
            .as_bytes(),
    );
    let signing_input = format!("{header}.{claims}");
    let sig = key_pair
        .sign(&rng, signing_input.as_bytes())
        .map_err(|e| format!("signing failed: {e}"))?;
    Ok(format!(
        "{signing_input}.{}",
        URL_SAFE_NO_PAD.encode(sig.as_ref())
    ))
}

// --- Token storage ---

/// Register (or refresh) a device token for an account. A token presented by a
/// different account moves to it — the device changed owner.
pub async fn register_token(
    pool: &PgPool,
    account_id: uuid::Uuid,
    device_token: &str,
    platform: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO push_tokens (account_id, device_token, platform) VALUES ($1, $2, $3) \
         ON CONFLICT (device_token) DO UPDATE \
         SET account_id = EXCLUDED.account_id, platform = EXCLUDED.platform, last_seen_at = NOW()",
    )
    .bind(account_id)
    .bind(device_token)
    .bind(platform)
    .execute(pool)
    .await?;
    Ok(())
}

/// Remove one of the account's device tokens (sign-out / token invalidation).
pub async fn remove_token(
    pool: &PgPool,
    account_id: uuid::Uuid,
    device_token: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM push_tokens WHERE account_id = $1 AND device_token = $2")
        .bind(account_id)
        .bind(device_token)
        .execute(pool)
        .await?;
    Ok(())
}

/// Device tokens registered to the account that owns `address`.
pub async fn tokens_for_address(pool: &PgPool, address: &str) -> Result<Vec<String>, sqlx::Error> {
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT pt.device_token FROM push_tokens pt \
         JOIN accounts a ON a.id = pt.account_id WHERE a.address = $1",
    )
    .bind(address)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(|(t,)| t).collect())
}

// --- Sender ---

/// The push delivery mechanism. `Noop` when APNs is unconfigured; `Capture`
/// feeds a channel for tests; `Apns` performs the real HTTP/2 delivery.
pub enum PushSender {
    Noop,
    Apns(ApnsClient),
    /// Test capture. Constructed only under cfg(test) in the binary tree and by
    /// lib integration tests, so the non-test binary target sees it as dead.
    #[allow(dead_code)]
    Capture(tokio::sync::mpsc::UnboundedSender<(String, serde_json::Value)>),
}

impl PushSender {
    /// Build from env: configured → `Apns`, otherwise `Noop`.
    pub fn from_env() -> Self {
        match settings_from_env() {
            Some(settings) => PushSender::Apns(ApnsClient::new(settings)),
            None => PushSender::Noop,
        }
    }

    pub fn is_enabled(&self) -> bool {
        !matches!(self, PushSender::Noop)
    }

    pub async fn send(&self, device_token: &str, payload: &serde_json::Value) {
        match self {
            PushSender::Noop => {}
            PushSender::Capture(tx) => {
                let _ = tx.send((device_token.to_string(), payload.clone()));
            }
            PushSender::Apns(client) => {
                if let Err(e) = client.send(device_token, payload).await {
                    tracing::warn!("[push] APNs delivery to {device_token} failed: {e}");
                }
            }
        }
    }
}

/// Minimal APNs HTTP/2 client (provider-token auth). Uses the workspace's
/// existing reqwest + ring; no APNs SDK dependency.
pub struct ApnsClient {
    settings: ApnsSettings,
    http: reqwest::Client,
}

impl ApnsClient {
    pub fn new(settings: ApnsSettings) -> Self {
        Self {
            settings,
            http: reqwest::Client::new(),
        }
    }

    async fn send(&self, device_token: &str, payload: &serde_json::Value) -> Result<(), String> {
        let pem = std::fs::read_to_string(&self.settings.key_path)
            .map_err(|e| format!("read {}: {e}", self.settings.key_path))?;
        let jwt = provider_jwt(
            &pem,
            &self.settings.key_id,
            &self.settings.team_id,
            chrono::Utc::now().timestamp(),
        )?;
        let url = format!("{}/3/device/{device_token}", self.settings.host());
        let resp = self
            .http
            .post(&url)
            .header("authorization", format!("bearer {jwt}"))
            .header("apns-topic", &self.settings.topic)
            .header("apns-push-type", "alert")
            .json(payload)
            .send()
            .await
            .map_err(|e| e.to_string())?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("APNs {status}: {body}"));
        }
        Ok(())
    }
}
