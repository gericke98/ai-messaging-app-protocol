//! Pure retrieval helpers for memory briefing: query-string building,
//! recency×confidence ranking, token-budget packing, peer-card assembly.
//! No DB or network here — all functions are pure and unit-tested.

const STOPWORDS: &[&str] = &[
    "the", "and", "for", "you", "your", "are", "but", "with", "this", "that",
    "have", "was", "not", "can", "will", "what", "how", "why", "who", "our",
];

/// Build a `to_tsquery`-safe OR query from recent message texts.
/// Returns None if no usable terms remain.
pub fn build_query_string(messages: &[String]) -> Option<String> {
    let mut seen = std::collections::BTreeSet::new();
    for m in messages {
        for raw in m.split(|c: char| !c.is_alphanumeric()) {
            let term = raw.to_lowercase();
            if term.len() >= 3 && !STOPWORDS.contains(&term.as_str()) {
                seen.insert(term);
            }
        }
    }
    if seen.is_empty() {
        return None;
    }
    Some(seen.into_iter().collect::<Vec<_>>().join(" | "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_query_string_extracts_or_joined_terms() {
        let msgs = vec![
            "I want help planning my Rust backend migration".to_string(),
            "the DB is Postgres".to_string(),
        ];
        let q = build_query_string(&msgs).unwrap();
        for term in ["rust", "backend", "migration", "postgres"] {
            assert!(q.contains(term), "expected `{term}` in `{q}`");
        }
        assert!(q.contains('|'));
        assert!(!q.contains("the"));
    }

    #[test]
    fn build_query_string_none_when_no_terms() {
        assert!(build_query_string(&["a an the".to_string()]).is_none());
        assert!(build_query_string(&[]).is_none());
    }
}
