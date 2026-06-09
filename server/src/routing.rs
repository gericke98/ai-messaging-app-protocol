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

/// Outcome of one A2A hop-counter transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HopDecision {
    /// New consecutive-A2A-hop count to store for the thread (0 = cleared).
    pub new_count: u32,
    /// Whether the target agent should be invoked. `false` = circuit tripped.
    pub deliver_to_target: bool,
    /// Whether THIS message tripped the cap (used to emit the one-shot notice).
    pub tripped: bool,
}

/// Pure A2A hop-counter transition for one inbound message in a thread.
///
/// `current` is the stored count of consecutive agent→agent hops with no human
/// message between, for this thread. `cap` is the configured maximum.
///
/// Rules:
/// - Non-agent (human) sender: resets the counter to 0 and always delivers.
/// - Agent sender, non-agent target (agent→human / no agent target): counter
///   unchanged, always delivers.
/// - Agent sender, agent target (the A2A case): if `current >= cap`, do NOT
///   deliver and report `tripped`; otherwise increment and deliver.
pub fn a2a_hop_decision(
    current: u32,
    sender_is_agent: bool,
    target_is_agent: bool,
    cap: u32,
) -> HopDecision {
    if !sender_is_agent {
        return HopDecision { new_count: 0, deliver_to_target: true, tripped: false };
    }
    if !target_is_agent {
        return HopDecision { new_count: current, deliver_to_target: true, tripped: false };
    }
    if current >= cap {
        return HopDecision { new_count: current, deliver_to_target: false, tripped: true };
    }
    HopDecision { new_count: current + 1, deliver_to_target: true, tripped: false }
}

/// Pure A2A consent predicate (P2). An agent→agent message may be delivered only when:
/// 1. the SENDER agent was human-invited into the thread,
/// 2. the TARGET agent was human-invited into the thread, AND
/// 3. the target has not opted out of peer messages (`acceptsA2A != false`).
///
/// Human-invited membership is established elsewhere (a human message targeting the agent);
/// this predicate is pure over the three resolved booleans.
pub fn consent_ok(
    from_is_human_invited: bool,
    to_is_human_invited: bool,
    to_accepts_a2a: bool,
) -> bool {
    from_is_human_invited && to_is_human_invited && to_accepts_a2a
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

    #[test]
    fn human_sender_resets_and_delivers() {
        let d = a2a_hop_decision(5, false, true, 6);
        assert_eq!(d, HopDecision { new_count: 0, deliver_to_target: true, tripped: false });
        // even targeting a human
        let d2 = a2a_hop_decision(3, false, false, 6);
        assert_eq!(d2.new_count, 0);
        assert!(d2.deliver_to_target);
    }

    #[test]
    fn agent_to_human_leaves_counter_unchanged() {
        let d = a2a_hop_decision(4, true, false, 6);
        assert_eq!(d, HopDecision { new_count: 4, deliver_to_target: true, tripped: false });
    }

    #[test]
    fn agent_to_agent_increments_until_cap() {
        let d = a2a_hop_decision(0, true, true, 6);
        assert_eq!(d, HopDecision { new_count: 1, deliver_to_target: true, tripped: false });
        let d5 = a2a_hop_decision(5, true, true, 6);
        assert_eq!(d5, HopDecision { new_count: 6, deliver_to_target: true, tripped: false });
    }

    #[test]
    fn agent_to_agent_trips_at_cap() {
        let d = a2a_hop_decision(6, true, true, 6);
        assert_eq!(d, HopDecision { new_count: 6, deliver_to_target: false, tripped: true });
        // and stays tripped above cap
        let d2 = a2a_hop_decision(9, true, true, 6);
        assert!(!d2.deliver_to_target);
        assert!(d2.tripped);
    }

    #[test]
    fn consent_requires_both_invited_and_accept() {
        assert!(consent_ok(true, true, true));
    }

    #[test]
    fn consent_denied_when_sender_not_invited() {
        assert!(!consent_ok(false, true, true));
    }

    #[test]
    fn consent_denied_when_target_not_invited() {
        assert!(!consent_ok(true, false, true));
    }

    #[test]
    fn consent_denied_when_target_opts_out() {
        assert!(!consent_ok(true, true, false));
    }
}
