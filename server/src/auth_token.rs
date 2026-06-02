//! Stateless, single-purpose, short-lived auth tokens (HMAC-SHA256).
//! Used to gate the enrollment and recovery REST steps after email verification.

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use ring::hmac;

#[derive(Clone)]
pub struct TokenSigner {
    key: hmac::Key,
}

#[derive(Debug, PartialEq, Eq)]
pub enum TokenError {
    Malformed,
    BadSignature,
    Expired,
    WrongPurpose,
}

impl TokenSigner {
    /// Build from raw secret bytes (from `AINM_TOKEN_SECRET` or a random startup value).
    pub fn new(secret: &[u8]) -> Self {
        Self { key: hmac::Key::new(hmac::HMAC_SHA256, secret) }
    }

    /// Issue a token for `account_id` + `purpose`, valid for `ttl_secs` from `now_unix`.
    pub fn issue(&self, account_id: &str, purpose: &str, now_unix: i64, ttl_secs: i64) -> String {
        let payload = format!("{account_id}|{purpose}|{}", now_unix + ttl_secs);
        let tag = hmac::sign(&self.key, payload.as_bytes());
        format!(
            "{}.{}",
            URL_SAFE_NO_PAD.encode(payload.as_bytes()),
            URL_SAFE_NO_PAD.encode(tag.as_ref())
        )
    }

    /// Verify a token's signature, purpose, and expiry. Returns the `account_id` on success.
    pub fn verify(&self, token: &str, expected_purpose: &str, now_unix: i64) -> Result<String, TokenError> {
        let (p_b64, t_b64) = token.split_once('.').ok_or(TokenError::Malformed)?;
        let payload = URL_SAFE_NO_PAD.decode(p_b64).map_err(|_| TokenError::Malformed)?;
        let tag = URL_SAFE_NO_PAD.decode(t_b64).map_err(|_| TokenError::Malformed)?;
        hmac::verify(&self.key, &payload, &tag).map_err(|_| TokenError::BadSignature)?;

        let payload_str = String::from_utf8(payload).map_err(|_| TokenError::Malformed)?;
        let mut parts = payload_str.split('|');
        let account_id = parts.next().ok_or(TokenError::Malformed)?;
        let purpose = parts.next().ok_or(TokenError::Malformed)?;
        let exp: i64 = parts.next().ok_or(TokenError::Malformed)?
            .parse().map_err(|_| TokenError::Malformed)?;

        if purpose != expected_purpose { return Err(TokenError::WrongPurpose); }
        if now_unix >= exp { return Err(TokenError::Expired); }
        Ok(account_id.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_and_returns_account() {
        let s = TokenSigner::new(b"test-secret");
        let tok = s.issue("acct-1", "enrollment", 1000, 900);
        assert_eq!(s.verify(&tok, "enrollment", 1100).unwrap(), "acct-1");
    }

    #[test]
    fn rejects_wrong_purpose_expiry_and_tamper() {
        let s = TokenSigner::new(b"test-secret");
        let tok = s.issue("acct-1", "enrollment", 1000, 900);
        assert_eq!(s.verify(&tok, "recovery", 1100), Err(TokenError::WrongPurpose));
        assert_eq!(s.verify(&tok, "enrollment", 2000), Err(TokenError::Expired));

        let other = TokenSigner::new(b"different-secret");
        assert_eq!(other.verify(&tok, "enrollment", 1100), Err(TokenError::BadSignature));
        assert_eq!(s.verify("garbage", "enrollment", 1100), Err(TokenError::Malformed));
    }
}
