//! Production configuration gate. When `AINM_ENV=production`, the server must refuse
//! to boot if required secrets/config are missing or unsafe, rather than silently
//! falling back (random token secret, BYO disabled, mock email/LLM). The validation
//! core is pure (`validate_with`) so it is testable without mutating process env.

use base64::Engine;

/// True when `AINM_ENV=production`.
pub fn is_production() -> bool {
    std::env::var("AINM_ENV").as_deref() == Ok("production")
}

/// Validate production config against the live process environment.
pub fn validate() -> Result<(), Vec<String>> {
    validate_with(&|k| std::env::var(k).ok())
}

/// Pure validation core. `get(name)` returns the env value or `None`.
/// Aggregates ALL problems so one boot attempt reports everything to fix.
/// Never echoes secret material — only names and length/format facts.
pub fn validate_with(get: &dyn Fn(&str) -> Option<String>) -> Result<(), Vec<String>> {
    let mut problems = Vec::new();

    match get("AINM_TOKEN_SECRET") {
        Some(s) if s.len() >= 32 => {}
        Some(_) => problems.push(
            "AINM_TOKEN_SECRET must be at least 32 bytes (used to sign enrollment/recovery tokens)".to_string(),
        ),
        None => problems.push("AINM_TOKEN_SECRET is required in production".to_string()),
    }

    match get("AINM_LLM_ENCRYPTION_KEY") {
        Some(b64) => match base64::engine::general_purpose::STANDARD.decode(b64.trim()) {
            Ok(bytes) if bytes.len() >= 32 => {}
            Ok(_) => problems.push(
                "AINM_LLM_ENCRYPTION_KEY must decode to at least 32 bytes (use: openssl rand -base64 32)".to_string(),
            ),
            Err(_) => problems.push("AINM_LLM_ENCRYPTION_KEY must be valid base64".to_string()),
        },
        None => problems.push("AINM_LLM_ENCRYPTION_KEY is required in production".to_string()),
    }

    if get("AINM_DATABASE__URL").map(|s| s.trim().is_empty()).unwrap_or(true) {
        problems.push("AINM_DATABASE__URL is required in production".to_string());
    }

    if get("AINM_LLM__API_KEY").map(|s| s.trim().is_empty()).unwrap_or(true) {
        problems.push(
            "AINM_LLM__API_KEY is required in production (otherwise the compilation worker and hosted agent silently degrade to a mock LLM)".to_string(),
        );
    }

    if get("AINM_LLM__MOCK")
        .map(|v| {
            let v = v.trim().to_ascii_lowercase();
            v == "1" || v == "true" || v == "yes" || v == "on"
        })
        .unwrap_or(false)
    {
        problems.push("AINM_LLM__MOCK must not be set in production".to_string());
    }

    let has_resend = get("AINM_EMAIL__RESEND_API_KEY").map(|s| !s.trim().is_empty()).unwrap_or(false);
    let has_from = get("AINM_EMAIL__FROM").map(|s| !s.trim().is_empty()).unwrap_or(false);
    if !(has_resend && has_from) {
        problems.push(
            "AINM_EMAIL__RESEND_API_KEY and AINM_EMAIL__FROM are both required in production (otherwise OTPs are only logged)".to_string(),
        );
    }

    if problems.is_empty() {
        Ok(())
    } else {
        Err(problems)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn good() -> HashMap<String, String> {
        let mut m = HashMap::new();
        m.insert("AINM_TOKEN_SECRET".into(), "x".repeat(32));
        // 32-byte key, base64 — satisfies the >= 32-byte minimum.
        m.insert(
            "AINM_LLM_ENCRYPTION_KEY".into(),
            base64::engine::general_purpose::STANDARD.encode([0u8; 32]),
        );
        m.insert("AINM_DATABASE__URL".into(), "postgres://u@h/db".into());
        m.insert("AINM_LLM__API_KEY".into(), "sk-test-123".into());
        m.insert("AINM_EMAIL__RESEND_API_KEY".into(), "re_123".into());
        m.insert("AINM_EMAIL__FROM".into(), "noreply@example.com".into());
        m
    }

    fn getter(m: HashMap<String, String>) -> impl Fn(&str) -> Option<String> {
        move |k: &str| m.get(k).cloned()
    }

    #[test]
    fn all_present_is_ok() {
        assert!(validate_with(&getter(good())).is_ok());
    }

    #[test]
    fn missing_token_secret_is_reported() {
        let mut m = good();
        m.remove("AINM_TOKEN_SECRET");
        let err = validate_with(&getter(m)).unwrap_err();
        assert!(err.iter().any(|p| p.contains("AINM_TOKEN_SECRET")));
    }

    #[test]
    fn short_token_secret_is_reported() {
        let mut m = good();
        m.insert("AINM_TOKEN_SECRET".into(), "short".into());
        let err = validate_with(&getter(m)).unwrap_err();
        assert!(err.iter().any(|p| p.contains("AINM_TOKEN_SECRET")));
    }

    #[test]
    fn invalid_encryption_key_base64_is_reported() {
        let mut m = good();
        m.insert("AINM_LLM_ENCRYPTION_KEY".into(), "not valid base64!!!".into());
        let err = validate_with(&getter(m)).unwrap_err();
        assert!(err.iter().any(|p| p.contains("AINM_LLM_ENCRYPTION_KEY")));
    }

    #[test]
    fn short_encryption_key_is_reported() {
        let mut m = good();
        // valid base64 but only 5 decoded bytes — below the 32-byte minimum.
        m.insert(
            "AINM_LLM_ENCRYPTION_KEY".into(),
            base64::engine::general_purpose::STANDARD.encode([0u8; 5]),
        );
        let err = validate_with(&getter(m)).unwrap_err();
        assert!(err.iter().any(|p| p.contains("AINM_LLM_ENCRYPTION_KEY")));
    }

    #[test]
    fn mock_llm_case_insensitive_rejected() {
        let mut m = good();
        m.insert("AINM_LLM__MOCK".into(), "True".into());
        let err = validate_with(&getter(m)).unwrap_err();
        assert!(err.iter().any(|p| p.contains("AINM_LLM__MOCK")));
    }

    #[test]
    fn missing_database_url_is_reported() {
        let mut m = good();
        m.remove("AINM_DATABASE__URL");
        let err = validate_with(&getter(m)).unwrap_err();
        assert!(err.iter().any(|p| p.contains("AINM_DATABASE__URL")));
    }

    #[test]
    fn mock_llm_in_production_is_rejected() {
        let mut m = good();
        m.insert("AINM_LLM__MOCK".into(), "1".into());
        let err = validate_with(&getter(m)).unwrap_err();
        assert!(err.iter().any(|p| p.contains("AINM_LLM__MOCK")));
    }

    #[test]
    fn missing_llm_api_key_is_reported() {
        let mut m = good();
        m.remove("AINM_LLM__API_KEY");
        let err = validate_with(&getter(m)).unwrap_err();
        assert!(err.iter().any(|p| p.contains("AINM_LLM__API_KEY")));
    }

    #[test]
    fn blank_llm_api_key_is_reported() {
        let mut m = good();
        m.insert("AINM_LLM__API_KEY".into(), "   ".into());
        let err = validate_with(&getter(m)).unwrap_err();
        assert!(err.iter().any(|p| p.contains("AINM_LLM__API_KEY")));
    }

    #[test]
    fn missing_email_config_is_reported() {
        let mut m = good();
        m.remove("AINM_EMAIL__FROM");
        let err = validate_with(&getter(m)).unwrap_err();
        assert!(err.iter().any(|p| p.contains("AINM_EMAIL")));
    }

    #[test]
    fn problems_aggregate_not_first_fail() {
        let mut m = good();
        m.remove("AINM_TOKEN_SECRET");
        m.remove("AINM_DATABASE__URL");
        let err = validate_with(&getter(m)).unwrap_err();
        assert!(err.iter().any(|p| p.contains("AINM_TOKEN_SECRET")));
        assert!(err.iter().any(|p| p.contains("AINM_DATABASE__URL")));
    }
}
