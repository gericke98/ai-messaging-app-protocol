//! Rate limiting (M2): GCRA per-client-IP on the production router.
//! Uses SmartIpKeyExtractor (x-forwarded-for / x-real-ip / forwarded, then peer
//! IP) so limits key on the real client behind Fly's proxy — which also makes
//! the layer testable with plain header-carrying oneshot requests.

use ainm_server::rate_limit;
use axum::body::Body;
use axum::http::Request;
use axum::routing::get;
use axum::Router;
use tower::ServiceExt;

fn app(replenish_ms: u64, burst: u32) -> Router {
    Router::new()
        .route("/", get(|| async { "ok" }))
        .layer(rate_limit::layer(replenish_ms, burst))
}

fn req(ip: &str) -> Request<Body> {
    Request::builder()
        .uri("/")
        .header("x-forwarded-for", ip)
        .body(Body::empty())
        .unwrap()
}

#[tokio::test]
async fn returns_429_once_burst_is_exhausted() {
    // Replenish 1 per minute, burst 3: requests 4..6 must be limited.
    let app = app(60_000, 3);
    let mut codes = Vec::new();
    for _ in 0..6 {
        let resp = app.clone().oneshot(req("9.9.9.9")).await.unwrap();
        codes.push(resp.status().as_u16());
    }
    assert_eq!(&codes[..3], &[200, 200, 200], "burst allowed: {codes:?}");
    assert!(
        codes[3..].iter().all(|c| *c == 429),
        "past burst limited: {codes:?}"
    );
}

#[tokio::test]
async fn distinct_client_ips_have_independent_budgets() {
    let app = app(60_000, 2);
    for _ in 0..2 {
        assert_eq!(
            app.clone().oneshot(req("1.1.1.1")).await.unwrap().status(),
            200
        );
    }
    assert_eq!(
        app.clone().oneshot(req("1.1.1.1")).await.unwrap().status(),
        429
    );
    // A different client is unaffected.
    assert_eq!(
        app.clone().oneshot(req("2.2.2.2")).await.unwrap().status(),
        200
    );
}

#[tokio::test]
async fn config_from_env_falls_back_to_defaults() {
    let get = |_: &str| None;
    let (replenish_ms, burst) = rate_limit::settings_with(&get);
    assert_eq!((replenish_ms, burst), (50, 100));
}

#[tokio::test]
async fn config_from_env_reads_overrides() {
    let get = |k: &str| match k {
        "AINM_RATE_LIMIT__REPLENISH_MS" => Some("10".to_string()),
        "AINM_RATE_LIMIT__BURST" => Some("500".to_string()),
        _ => None,
    };
    let (replenish_ms, burst) = rate_limit::settings_with(&get);
    assert_eq!((replenish_ms, burst), (10, 500));
}
