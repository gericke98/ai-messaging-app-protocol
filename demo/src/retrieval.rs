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

use chrono::{DateTime, Utc};

const RECENCY_HALF_LIFE_DAYS: f64 = 30.0;

/// Rank score for a fact: confidence weighted by exponential recency decay.
pub fn score_node(valid_from: DateTime<Utc>, now: DateTime<Utc>, confidence: f32) -> f32 {
    let age_days = (now - valid_from).num_seconds().max(0) as f64 / 86_400.0;
    let recency = 0.5_f64.powf(age_days / RECENCY_HALF_LIFE_DAYS);
    (confidence as f64 * recency) as f32
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};

    #[test]
    fn score_node_newer_outranks_older_at_equal_confidence() {
        let now = Utc.with_ymd_and_hms(2026, 5, 30, 0, 0, 0).unwrap();
        let recent = Utc.with_ymd_and_hms(2026, 5, 29, 0, 0, 0).unwrap();
        let old = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        assert!(score_node(recent, now, 0.9) > score_node(old, now, 0.9));
    }

    #[test]
    fn score_node_higher_confidence_outranks_at_equal_age() {
        let now = Utc.with_ymd_and_hms(2026, 5, 30, 0, 0, 0).unwrap();
        let t = Utc.with_ymd_and_hms(2026, 5, 20, 0, 0, 0).unwrap();
        assert!(score_node(t, now, 0.9) > score_node(t, now, 0.4));
    }

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
