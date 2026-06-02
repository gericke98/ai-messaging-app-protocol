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

#[derive(Debug, Clone, PartialEq)]
pub struct RankedItem {
    pub id: i64,
    pub score: f32,
    pub char_len: usize,
}

/// Greedy budget fill among items scoring >= min_score: sort by score desc,
/// keep while cumulative char_len <= budget. Returns kept ids in score order.
pub fn pack_to_budget(mut items: Vec<RankedItem>, budget_chars: usize, min_score: f32) -> Vec<i64> {
    items.retain(|it| it.score >= min_score);
    items.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    let mut used = 0usize;
    let mut kept = Vec::new();
    for it in items {
        if used + it.char_len <= budget_chars {
            used += it.char_len;
            kept.push(it.id);
        }
    }
    kept
}

/// Build a capped, human-readable identity anchor from fact labels.
/// Adds facts as "- <label>\n" while staying within `cap_chars`. Empty if none fit.
pub fn build_peer_card(facts: &[String], cap_chars: usize) -> String {
    let mut out = String::new();
    for f in facts {
        let line = format!("- {f}\n");
        if out.len() + line.len() <= cap_chars {
            out.push_str(&line);
        }
    }
    out.trim_end().to_string()
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

    #[test]
    fn pack_to_budget_keeps_highest_scores_within_budget() {
        let items = vec![
            RankedItem { id: 1, score: 0.2, char_len: 50 },
            RankedItem { id: 2, score: 0.9, char_len: 50 },
            RankedItem { id: 3, score: 0.5, char_len: 50 },
        ];
        let kept = pack_to_budget(items, 100, 0.0);
        assert_eq!(kept, vec![2, 3]);
    }

    #[test]
    fn pack_to_budget_skips_overflow_keeps_later_smaller() {
        let items = vec![
            RankedItem { id: 1, score: 0.9, char_len: 80 },
            RankedItem { id: 2, score: 0.8, char_len: 50 },
            RankedItem { id: 3, score: 0.7, char_len: 15 },
        ];
        let kept = pack_to_budget(items, 100, 0.0);
        assert_eq!(kept, vec![1, 3]);
    }

    #[test]
    fn pack_to_budget_drops_below_floor_even_with_budget_room() {
        let items = vec![
            RankedItem { id: 1, score: 0.9, char_len: 10 },
            RankedItem { id: 2, score: 0.001, char_len: 10 }, // below floor, budget has room
        ];
        let kept = pack_to_budget(items, 1000, 0.05);
        assert_eq!(kept, vec![1]); // id 2 dropped by floor despite ample budget
    }

    #[test]
    fn build_peer_card_joins_facts_under_cap() {
        let facts = vec![
            "Name is Santiago".to_string(),
            "Prefers Rust".to_string(),
            "Building AINM".to_string(),
        ];
        let card = build_peer_card(&facts, 200);
        assert!(card.contains("Santiago") && card.contains("Rust") && card.contains("AINM"));
    }

    #[test]
    fn build_peer_card_truncates_whole_facts_to_cap() {
        let facts = vec!["a".repeat(40), "b".repeat(40), "c".repeat(40)];
        let card = build_peer_card(&facts, 90);
        assert!(card.len() <= 90);
        assert!(card.contains(&"a".repeat(40)));
    }

    #[test]
    fn build_peer_card_empty_when_no_facts() {
        assert_eq!(build_peer_card(&[], 200), "");
    }
}
