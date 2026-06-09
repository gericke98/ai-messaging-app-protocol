//! Per-client-IP rate limiting (M2): GCRA via tower_governor.
//!
//! Keyed with `SmartIpKeyExtractor` (x-forwarded-for / x-real-ip / forwarded
//! headers, then peer IP) because production sits behind Fly's proxy — peer-IP
//! keying there would put every client in one bucket.

use axum::body::Body;
use std::sync::Arc;
use tower_governor::governor::GovernorConfigBuilder;
use tower_governor::key_extractor::SmartIpKeyExtractor;
use tower_governor::GovernorLayer;

type SmartGovernorLayer = GovernorLayer<
    SmartIpKeyExtractor,
    governor::middleware::NoOpMiddleware<governor::clock::QuantaInstant>,
    Body,
>;

/// One token replenished every `replenish_ms`, with a bucket of `burst` —
/// e.g. (50, 100) = sustained 20 req/s per client with bursts up to 100.
pub fn layer(replenish_ms: u64, burst: u32) -> SmartGovernorLayer {
    let config = GovernorConfigBuilder::default()
        .per_millisecond(replenish_ms)
        .burst_size(burst)
        .key_extractor(SmartIpKeyExtractor)
        .finish()
        .expect("non-zero rate limit parameters");
    GovernorLayer::new(Arc::new(config))
}

/// Resolve (replenish_ms, burst) from the live environment.
pub fn settings_from_env() -> (u64, u32) {
    settings_with(&|k| std::env::var(k).ok())
}

/// Pure core of `settings_from_env` — `get(name)` returns the env value or `None`.
/// Defaults: 20 req/s sustained, burst 100 — generous for a messaging client
/// doing initial sync, tight enough to blunt abuse.
pub fn settings_with(get: &dyn Fn(&str) -> Option<String>) -> (u64, u32) {
    let replenish_ms = get("AINM_RATE_LIMIT__REPLENISH_MS")
        .and_then(|v| v.parse().ok())
        .filter(|v: &u64| *v > 0)
        .unwrap_or(50);
    let burst = get("AINM_RATE_LIMIT__BURST")
        .and_then(|v| v.parse().ok())
        .filter(|v: &u32| *v > 0)
        .unwrap_or(100);
    (replenish_ms, burst)
}
