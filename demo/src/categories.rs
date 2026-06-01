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
}
