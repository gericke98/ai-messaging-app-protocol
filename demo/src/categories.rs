//! Canonical compound-memory category vocabulary — the single source of truth.
//!
//! These are the only categories the compiler may emit. The three entity-type
//! groups are the source; `all()` derives the full closed set from them so they
//! cannot drift apart. `briefing::scope_to_categories` must map every category
//! here (enforced by a completeness test in `briefing.rs`).

/// Stable facts about the user.
pub const SEMANTIC: &[&str] = &["fact", "preference", "skill", "relationship", "tool"];
/// Time-bound events.
pub const EPISODIC: &[&str] = &["event", "decision", "outcome", "interaction"];
/// Learned patterns / procedures.
pub const PROCEDURAL: &[&str] = &["pattern", "workflow", "strategy", "communication"];

/// Every category the compiler may emit (closed set), in stable order.
pub fn all() -> Vec<&'static str> {
    SEMANTIC
        .iter()
        .chain(EPISODIC)
        .chain(PROCEDURAL)
        .copied()
        .collect()
}

/// Normalize a raw category to its canonical form, or `None` if off-list.
///
/// Trims surrounding whitespace and ASCII-lowercases the input, then
/// membership-tests against the closed canonical set. Returns the canonical
/// `&'static str` literal so callers store the exact vocabulary form rather
/// than the model's casing.
pub fn normalize(raw: &str) -> Option<&'static str> {
    let key = raw.trim().to_ascii_lowercase();
    all().into_iter().find(|&c| c == key.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn all_is_the_union_of_groups_with_no_duplicates() {
        let all = all();
        assert_eq!(
            all.len(),
            SEMANTIC.len() + EPISODIC.len() + PROCEDURAL.len(),
            "all() must be exactly the three groups concatenated"
        );
        assert_eq!(all.len(), 13, "canonical set is 13 categories");
        let unique: BTreeSet<&&str> = all.iter().collect();
        assert_eq!(unique.len(), all.len(), "no category may appear in two groups");
    }

    #[test]
    fn normalize_accepts_exact_canonical() {
        assert_eq!(normalize("fact"), Some("fact"));
        assert_eq!(normalize("decision"), Some("decision"));
        assert_eq!(normalize("communication"), Some("communication"));
    }

    #[test]
    fn normalize_recovers_casing_and_whitespace_variants() {
        assert_eq!(normalize("Fact"), Some("fact"));
        assert_eq!(normalize(" preference "), Some("preference"));
        assert_eq!(normalize("DECISION"), Some("decision"));
        assert_eq!(normalize("Skill"), Some("skill"));
    }

    #[test]
    fn normalize_rejects_off_list_and_empty() {
        assert_eq!(normalize("project"), None);
        assert_eq!(normalize("misc"), None);
        assert_eq!(normalize(""), None);
        assert_eq!(normalize("   "), None);
    }
}
