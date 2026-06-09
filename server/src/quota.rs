//! Per-requester monthly token usage meter (the llm_usage ledger).

use crate::error::StorageError;
use sqlx::PgPool;
use uuid::Uuid;

/// The 'YYYY-MM' period bucket for a UTC timestamp (pure; caller supplies the clock).
pub fn current_period(now: chrono::DateTime<chrono::Utc>) -> String {
    now.format("%Y-%m").to_string()
}

/// The effective budget: per-account override if set, else the config default.
pub fn resolve_budget(override_opt: Option<i64>, config_default: i64) -> i64 {
    override_opt.unwrap_or(config_default)
}

/// Current tokens used by a requester this period (0 if no row yet).
pub async fn current_usage(
    pool: &PgPool,
    requester: &str,
    period: &str,
) -> Result<i64, StorageError> {
    let used: Option<i64> = sqlx::query_scalar(
        "SELECT used_tokens FROM llm_usage WHERE requester_address = $1 AND period = $2",
    )
    .bind(requester)
    .bind(period)
    .fetch_optional(pool)
    .await?;
    Ok(used.unwrap_or(0))
}

/// Atomically add `tokens` to the requester's usage for `period`; returns the new total.
/// TOCTOU-safe upsert (escrow-counter shape).
pub async fn debit(
    pool: &PgPool,
    requester: &str,
    account_id: Option<Uuid>,
    period: &str,
    tokens: i64,
) -> Result<i64, StorageError> {
    let total: i64 = sqlx::query_scalar(
        "INSERT INTO llm_usage (requester_address, period, account_id, used_tokens) \
         VALUES ($1, $2, $3, $4) \
         ON CONFLICT (requester_address, period) \
         DO UPDATE SET used_tokens = llm_usage.used_tokens + EXCLUDED.used_tokens, updated_at = NOW() \
         RETURNING used_tokens",
    )
    .bind(requester).bind(period).bind(account_id).bind(tokens)
    .fetch_one(pool).await?;
    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn period_is_year_month_utc() {
        let t = chrono::DateTime::parse_from_rfc3339("2026-06-02T13:00:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc);
        assert_eq!(current_period(t), "2026-06");
        let t2 = chrono::DateTime::parse_from_rfc3339("2026-12-31T23:59:59Z")
            .unwrap()
            .with_timezone(&chrono::Utc);
        assert_eq!(current_period(t2), "2026-12");
    }

    #[test]
    fn budget_prefers_override() {
        assert_eq!(resolve_budget(Some(50_000), 200_000), 50_000);
        assert_eq!(resolve_budget(None, 200_000), 200_000);
    }
}
