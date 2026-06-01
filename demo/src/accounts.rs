//! Account lifecycle: username validation (pure) + account/key repository queries (DB).

/// Reserved usernames that may never be registered.
const RESERVED: &[&str] = &[
    "admin", "administrator", "system", "compiler", "support",
    "root", "ainm", "server", "help", "abuse", "security", "postmaster",
];

#[derive(Debug, PartialEq, Eq)]
pub enum UsernameError {
    TooShort,
    TooLong,
    InvalidChars,
    Reserved,
}

/// Validate a username against the v1 rules: `[a-z0-9_]`, 3–30 chars, not reserved.
/// Input is lowercased before checking (usernames are case-insensitive).
pub fn validate_username(raw: &str) -> Result<String, UsernameError> {
    let name = raw.trim().to_ascii_lowercase();
    if name.len() < 3 { return Err(UsernameError::TooShort); }
    if name.len() > 30 { return Err(UsernameError::TooLong); }
    if !name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_') {
        return Err(UsernameError::InvalidChars);
    }
    if RESERVED.contains(&name.as_str()) { return Err(UsernameError::Reserved); }
    Ok(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_valid_and_normalizes_case() {
        assert_eq!(validate_username("Santi").unwrap(), "santi");
        assert_eq!(validate_username("  user_42 ").unwrap(), "user_42");
    }

    #[test]
    fn rejects_length_and_charset() {
        assert_eq!(validate_username("ab"), Err(UsernameError::TooShort));
        assert_eq!(validate_username(&"x".repeat(31)), Err(UsernameError::TooLong));
        assert_eq!(validate_username("bad name"), Err(UsernameError::InvalidChars));
        assert_eq!(validate_username("bad-name"), Err(UsernameError::InvalidChars));
    }

    #[test]
    fn rejects_reserved() {
        assert_eq!(validate_username("admin"), Err(UsernameError::Reserved));
        assert_eq!(validate_username("Compiler"), Err(UsernameError::Reserved));
    }
}
