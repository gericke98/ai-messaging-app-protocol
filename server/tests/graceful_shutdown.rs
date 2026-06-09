//! Verifies the graceful-drain primitive: a turn already in flight when shutdown
//! is triggered completes before `wait_idle` returns, and the bounded timeout
//! fires when a turn overruns the deadline.

use ainm_server::lifecycle::ShutdownController;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

#[tokio::test]
async fn drain_waits_for_in_flight_turn() {
    let controller = ShutdownController::new();
    let done = Arc::new(AtomicBool::new(false));
    let done2 = done.clone();

    controller.spawn_turn(async move {
        tokio::time::sleep(Duration::from_millis(100)).await;
        done2.store(true, Ordering::SeqCst);
    });

    controller.trigger();
    tokio::time::timeout(Duration::from_secs(5), controller.wait_idle())
        .await
        .expect("drains within deadline");
    assert!(
        done.load(Ordering::SeqCst),
        "in-flight turn ran to completion"
    );
}

#[tokio::test]
async fn drain_deadline_bounds_an_overrunning_turn() {
    let controller = ShutdownController::new();
    controller.spawn_turn(async {
        tokio::time::sleep(Duration::from_secs(30)).await;
    });
    controller.trigger();
    let r = tokio::time::timeout(Duration::from_millis(150), controller.wait_idle()).await;
    assert!(r.is_err(), "wait_idle is bounded by the caller's timeout");
}
