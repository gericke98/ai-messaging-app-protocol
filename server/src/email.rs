//! Email delivery abstraction (mirrors llm::LlmClient/MockLlmClient) + OTP helpers.

use crate::error::StorageError;
use sha2::{Digest, Sha256};
use sqlx::PgPool;

/// Sends a numeric verification code to an email address.
#[async_trait::async_trait]
pub trait EmailSender: Send + Sync {
    async fn send_code(&self, to_email: &str, code: &str, purpose: &str) -> Result<(), String>;
}

/// Test/dev sender: records every (email, code, purpose) and never hits the network.
pub struct MockEmailSender {
    pub sent: std::sync::Mutex<Vec<(String, String, String)>>,
}

impl MockEmailSender {
    pub fn new() -> Self {
        Self {
            sent: std::sync::Mutex::new(Vec::new()),
        }
    }
    /// The most recently sent code for an email+purpose, if any.
    // Test-only helper: used by integration tests, not the server binary.
    #[allow(dead_code)]
    pub fn last_code(&self, email: &str, purpose: &str) -> Option<String> {
        self.sent
            .lock()
            .unwrap()
            .iter()
            .rev()
            .find(|(e, _, p)| e == email && p == purpose)
            .map(|(_, c, _)| c.clone())
    }
}

impl Default for MockEmailSender {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl EmailSender for MockEmailSender {
    async fn send_code(&self, to_email: &str, code: &str, purpose: &str) -> Result<(), String> {
        tracing::info!("[email:mock] code {code} -> {to_email} ({purpose})");
        self.sent.lock().unwrap().push((
            to_email.to_string(),
            code.to_string(),
            purpose.to_string(),
        ));
        Ok(())
    }
}

/// Minimal real sender (Resend HTTP API). Used only when configured; never in CI.
pub struct ResendEmailSender {
    http: reqwest::Client,
    api_key: String,
    from: String,
}

impl ResendEmailSender {
    pub fn new(api_key: String, from: String) -> Self {
        Self {
            http: reqwest::Client::new(),
            api_key,
            from,
        }
    }
}

#[async_trait::async_trait]
impl EmailSender for ResendEmailSender {
    async fn send_code(&self, to_email: &str, code: &str, purpose: &str) -> Result<(), String> {
        let subject = format!("Your AINM {purpose} code: {code}");
        let body = serde_json::json!({
            "from": self.from,
            "to": [to_email],
            "subject": subject,
            "text": format!("Your code is {code}. It expires in 10 minutes."),
        });
        let resp = self
            .http
            .post("https://api.resend.com/emails")
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await
            .map_err(|e| e.to_string())?;
        if !resp.status().is_success() {
            return Err(format!("resend returned {}", resp.status()));
        }
        Ok(())
    }
}

/// Hash an email (lowercased, trimmed) for storage/lookup. Hex sha256.
pub fn hash_email(email: &str) -> String {
    let mut h = Sha256::new();
    h.update(email.trim().to_ascii_lowercase().as_bytes());
    hex::encode(h.finalize())
}

/// Hash an OTP code for storage. Hex sha256.
pub fn hash_code(code: &str) -> String {
    let mut h = Sha256::new();
    h.update(code.as_bytes());
    hex::encode(h.finalize())
}

/// Generate a zero-padded 6-digit numeric OTP.
pub fn generate_code() -> String {
    use rand::Rng;
    let n: u32 = rand::thread_rng().gen_range(0..1_000_000);
    format!("{n:06}")
}

/// Insert a verification row (caller has already hashed code+email). 10-minute TTL.
pub async fn store_verification(
    pool: &PgPool,
    email_hash: &str,
    code_hash: &str,
    purpose: &str,
) -> Result<(), StorageError> {
    sqlx::query(
        "INSERT INTO email_verifications (email_hash, code_hash, purpose, expires_at) \
         VALUES ($1, $2, $3, NOW() + INTERVAL '10 minutes')",
    )
    .bind(email_hash)
    .bind(code_hash)
    .bind(purpose)
    .execute(pool)
    .await?;
    Ok(())
}

/// Outcome of verifying a code.
#[derive(Debug, PartialEq, Eq)]
pub enum VerifyOutcome {
    Ok,
    WrongCode,
    Expired,
    TooManyAttempts,
    NotFound,
}

const MAX_OTP_ATTEMPTS: i32 = 5;

/// Verify the latest unconsumed code for (email_hash, purpose). On success, marks it consumed.
/// On a wrong code, increments attempts; past MAX_OTP_ATTEMPTS returns TooManyAttempts.
pub async fn verify_code(
    pool: &PgPool,
    email_hash: &str,
    purpose: &str,
    code_hash: &str,
) -> Result<VerifyOutcome, StorageError> {
    // Newest unconsumed row for this email+purpose.
    let row: Option<(uuid::Uuid, String, i32, bool)> = sqlx::query_as(
        "SELECT id, code_hash, attempts, (expires_at < NOW()) AS expired \
         FROM email_verifications \
         WHERE email_hash = $1 AND purpose = $2 AND consumed_at IS NULL \
         ORDER BY created_at DESC LIMIT 1",
    )
    .bind(email_hash)
    .bind(purpose)
    .fetch_optional(pool)
    .await?;

    let Some((id, stored_hash, attempts, expired)) = row else {
        return Ok(VerifyOutcome::NotFound);
    };
    if attempts >= MAX_OTP_ATTEMPTS {
        return Ok(VerifyOutcome::TooManyAttempts);
    }
    if expired {
        return Ok(VerifyOutcome::Expired);
    }
    if stored_hash != code_hash {
        sqlx::query("UPDATE email_verifications SET attempts = attempts + 1 WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        return Ok(VerifyOutcome::WrongCode);
    }
    sqlx::query("UPDATE email_verifications SET consumed_at = NOW() WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(VerifyOutcome::Ok)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn mock_records_codes() {
        let m = MockEmailSender::new();
        m.send_code("a@b.com", "123456", "signup").await.unwrap();
        assert_eq!(m.last_code("a@b.com", "signup").as_deref(), Some("123456"));
        assert_eq!(m.last_code("a@b.com", "recovery"), None);
    }

    #[test]
    fn email_hash_is_case_insensitive() {
        assert_eq!(hash_email("  Foo@Bar.com "), hash_email("foo@bar.com"));
    }

    #[test]
    fn generate_code_is_six_digits() {
        for _ in 0..50 {
            let c = generate_code();
            assert_eq!(c.len(), 6);
            assert!(c.chars().all(|ch| ch.is_ascii_digit()));
        }
    }
}
