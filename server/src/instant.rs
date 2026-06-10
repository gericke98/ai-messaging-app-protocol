//! Instant agent registration (M5 connector fast path).
//!
//! BotFather-style relaxation of the strict signup ceremony (email OTP +
//! key-bind): one POST creates an ACTIVE agent account with the supplied
//! Ed25519 public key already bound, so the signed WS handshake works
//! immediately and unchanged. Abuse pressure is absorbed by the per-IP rate
//! limiter; `AINM_INSTANT_REGISTRATION=0|false` is the kill switch.

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use sqlx::PgPool;

use crate::error::StorageError;

#[derive(Debug)]
pub enum InstantError {
    InvalidName,
    InvalidKey,
    /// Key already bound to another account (keys are globally unique).
    KeyInUse,
    Storage(StorageError),
}

impl std::fmt::Display for InstantError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidName => write!(f, "name must contain letters/digits (a-z, 0-9, -)"),
            Self::InvalidKey => write!(f, "public_key must be 32 base64url bytes"),
            Self::KeyInUse => write!(f, "public key already registered"),
            Self::Storage(e) => write!(f, "storage error: {e}"),
        }
    }
}

/// True unless `AINM_INSTANT_REGISTRATION` is `0`/`false`/`off`/`no`.
pub fn enabled_with(get: &dyn Fn(&str) -> Option<String>) -> bool {
    !matches!(
        get("AINM_INSTANT_REGISTRATION")
            .map(|v| v.trim().to_ascii_lowercase())
            .as_deref(),
        Some("0") | Some("false") | Some("off") | Some("no")
    )
}

pub fn enabled() -> bool {
    enabled_with(&|k| std::env::var(k).ok())
}

/// Lowercase, map runs of non-[a-z0-9-] to `-`, trim separators, cap at 32.
pub fn sanitize_name(raw: &str) -> Option<String> {
    let mut out = String::new();
    for c in raw.to_ascii_lowercase().chars() {
        if c.is_ascii_alphanumeric() || c == '-' {
            out.push(c);
        } else if !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
    }
    let trimmed: String = out.trim_matches('-').chars().take(32).collect();
    let trimmed = trimmed.trim_matches('-').to_string();
    if trimmed.chars().any(|c| c.is_ascii_alphanumeric()) {
        Some(trimmed)
    } else {
        None
    }
}

/// Create an active agent account named `name@domain` (auto-suffixed on
/// collision) with `public_key_b64` bound. Returns the final address.
pub async fn instant_register(
    pool: &PgPool,
    domain: &str,
    name: &str,
    public_key_b64: &str,
) -> Result<String, InstantError> {
    let base = sanitize_name(name).ok_or(InstantError::InvalidName)?;
    let key_bytes = URL_SAFE_NO_PAD
        .decode(public_key_b64)
        .map_err(|_| InstantError::InvalidKey)?;
    if key_bytes.len() != 32 {
        return Err(InstantError::InvalidKey);
    }

    // Try the requested name, then random-suffixed variants on collision.
    for attempt in 0..4 {
        let username = if attempt == 0 {
            base.clone()
        } else {
            let suffix: String = format!("{:08x}", uuid::Uuid::new_v4().as_fields().0)
                .chars()
                .take(4)
                .collect();
            format!("{}-{suffix}", base.chars().take(27).collect::<String>())
        };
        // Instant accounts have no email; the unique email_hash slot gets a
        // synthetic per-account value.
        let email_hash = format!("instant:{}", uuid::Uuid::new_v4());

        let acct =
            match crate::accounts::create_pending_account(pool, &username, domain, &email_hash)
                .await
            {
                Ok(a) => a,
                Err(StorageError::Duplicate(_)) => continue,
                Err(e) => return Err(InstantError::Storage(e)),
            };

        crate::accounts::set_status(pool, acct.id, "active")
            .await
            .map_err(InstantError::Storage)?;

        return match crate::accounts::insert_account_key(pool, acct.id, public_key_b64, None).await
        {
            Ok(()) => Ok(acct.address),
            // Keys are globally unique; a reused key must fail loudly, not
            // half-register (account row stays; harmless and auditable).
            Err(StorageError::Duplicate(_)) => Err(InstantError::KeyInUse),
            Err(e) => Err(InstantError::Storage(e)),
        };
    }
    Err(InstantError::Storage(StorageError::Duplicate(format!(
        "could not find a free name for {base}"
    ))))
}
