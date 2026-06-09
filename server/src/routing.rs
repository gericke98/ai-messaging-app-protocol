//! Pure routing helpers for runtime A2A (P1, same-server core).
//!
//! No I/O, no registries, no async — just the decision logic that says *who* a
//! message is addressed to and *whether* an agent→agent hop may proceed. Kept
//! pure so it is exhaustively unit-testable; the orchestrator in `server.rs`
//! supplies the live participant list / counters.

/// Local-part (before `@`) of an address. `"a@b"` → `"a"`; `"a"` → `"a"`.
fn local_part(addr: &str) -> &str {
    addr.split('@').next().unwrap_or(addr)
}

/// Extract the local-part of the FIRST `@mention` in `text`, if any.
///
/// A mention is `@` immediately followed by 1+ chars of `[A-Za-z0-9._-]`. The
/// returned string excludes the leading `@` (e.g. `"hey @analyst go"` → `Some("analyst")`).
/// `@` with no valid char after it is ignored.
pub fn first_mention_localpart(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'@' {
            let start = i + 1;
            let mut j = start;
            while j < bytes.len() {
                let c = bytes[j];
                if c.is_ascii_alphanumeric() || c == b'.' || c == b'_' || c == b'-' {
                    j += 1;
                } else {
                    break;
                }
            }
            if j > start {
                return Some(text[start..j].to_string());
            }
        }
        i += 1;
    }
    None
}

/// Resolve the routing target for a message.
///
/// 1. An explicit non-empty `to` wins verbatim.
/// 2. Otherwise the first `@mention` whose local-part matches a known
///    participant's local-part resolves to that participant's full address.
/// 3. Otherwise `None` (no agent target; humans observe).
pub fn resolve_recipient(
    to: Option<&str>,
    content_text: &str,
    known_participants: &[String],
) -> Option<String> {
    if let Some(t) = to {
        if !t.is_empty() {
            return Some(t.to_string());
        }
    }
    let mention = first_mention_localpart(content_text)?;
    known_participants
        .iter()
        .find(|p| local_part(p) == mention)
        .cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_mention_extracts_localpart() {
        assert_eq!(first_mention_localpart("hey @analyst please"), Some("analyst".to_string()));
        assert_eq!(first_mention_localpart("@bob"), Some("bob".to_string()));
        assert_eq!(first_mention_localpart("a.b_c-d @x.y_z-1 end"), Some("x.y_z-1".to_string()));
    }

    #[test]
    fn first_mention_none_when_absent_or_bare() {
        assert_eq!(first_mention_localpart("no mention here"), None);
        assert_eq!(first_mention_localpart("trailing @"), None);
        assert_eq!(first_mention_localpart("email like a@b is still a mention"), Some("b".to_string()));
    }

    #[test]
    fn resolve_prefers_explicit_to() {
        let parts = vec!["analyst@acme".to_string(), "writer@acme".to_string()];
        assert_eq!(
            resolve_recipient(Some("writer@acme"), "ignored @analyst", &parts),
            Some("writer@acme".to_string())
        );
    }

    #[test]
    fn resolve_empty_to_falls_through_to_mention() {
        let parts = vec!["analyst@acme".to_string()];
        assert_eq!(
            resolve_recipient(Some(""), "ping @analyst", &parts),
            Some("analyst@acme".to_string())
        );
    }

    #[test]
    fn resolve_mention_matches_localpart_to_full_address() {
        let parts = vec!["analyst@acme.test".to_string(), "alice@localhost".to_string()];
        assert_eq!(
            resolve_recipient(None, "could @analyst weigh in", &parts),
            Some("analyst@acme.test".to_string())
        );
    }

    #[test]
    fn resolve_none_when_no_to_and_mention_unknown() {
        let parts = vec!["analyst@acme".to_string()];
        assert_eq!(resolve_recipient(None, "hi @ghost", &parts), None);
        assert_eq!(resolve_recipient(None, "plain text", &parts), None);
        assert_eq!(resolve_recipient(None, "plain text", &[]), None);
    }
}
