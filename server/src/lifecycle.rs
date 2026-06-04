//! Process lifecycle: graceful shutdown signalling + in-flight agent-turn draining.
//!
//! `ShutdownController` is the single coordination point. A `watch<bool>` carries the
//! "shutting down" signal to every WebSocket connection loop and to each `axum::serve`
//! call's `with_graceful_shutdown` future. A `tokio_util::task::TaskTracker` counts
//! in-flight agent turns (which are spawned as detached tasks in `server.rs`), so the
//! drain can wait for a turn already underway to deliver its signed reply before exit.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::watch;
use tokio_util::task::TaskTracker;

/// Cloneable handle coordinating graceful shutdown across the server.
#[derive(Clone)]
pub struct ShutdownController {
    tx: Arc<watch::Sender<bool>>,
    flag: Arc<AtomicBool>,
    tracker: TaskTracker,
}

impl ShutdownController {
    pub fn new() -> Self {
        let (tx, _rx) = watch::channel(false);
        Self {
            tx: Arc::new(tx),
            flag: Arc::new(AtomicBool::new(false)),
            tracker: TaskTracker::new(),
        }
    }

    /// A receiver that observes the shutdown signal. Hand to `wait_for_shutdown`
    /// or `select!` on its `changed()`.
    pub fn subscribe(&self) -> watch::Receiver<bool> {
        self.tx.subscribe()
    }

    /// Signal shutdown. Idempotent.
    pub fn trigger(&self) {
        self.flag.store(true, Ordering::SeqCst);
        let _ = self.tx.send(true);
    }

    pub fn is_shutting_down(&self) -> bool {
        self.flag.load(Ordering::SeqCst)
    }

    /// Spawn an in-flight agent turn as a tracked task. The drain (`wait_idle`)
    /// waits for every spawned turn to finish. Use this instead of `tokio::spawn`
    /// for agent turns so a turn already underway at shutdown still completes.
    pub fn spawn_turn<F>(&self, fut: F)
    where
        F: std::future::Future<Output = ()> + Send + 'static,
    {
        self.tracker.spawn(fut);
    }

    /// Close the tracker and await all in-flight turns. Idempotent: calling twice
    /// is safe (the second call observes an already-closed, possibly-empty tracker).
    pub async fn wait_idle(&self) {
        self.tracker.close();
        self.tracker.wait().await;
    }
}

impl Default for ShutdownController {
    fn default() -> Self {
        Self::new()
    }
}

/// Resolve once the shutdown signal becomes `true` (including if already set).
pub async fn wait_for_shutdown(mut rx: watch::Receiver<bool>) {
    if *rx.borrow() {
        return;
    }
    while rx.changed().await.is_ok() {
        if *rx.borrow_and_update() {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn trigger_flips_flag_and_is_idempotent() {
        let c = ShutdownController::new();
        assert!(!c.is_shutting_down());
        c.trigger();
        assert!(c.is_shutting_down());
        c.trigger(); // second call must not panic
        assert!(c.is_shutting_down());
    }

    #[tokio::test]
    async fn wait_for_shutdown_returns_once_triggered() {
        let c = ShutdownController::new();
        let rx = c.subscribe();
        c.trigger();
        tokio::time::timeout(Duration::from_millis(100), wait_for_shutdown(rx))
            .await
            .expect("wait_for_shutdown returns promptly after trigger");
    }

    #[tokio::test]
    async fn wait_idle_pending_until_turn_completes() {
        let c = ShutdownController::new();
        let (tx, rx) = tokio::sync::oneshot::channel::<()>();
        c.spawn_turn(async move {
            let _ = rx.await;
        });
        // While the turn is alive, the drain must not complete.
        assert!(
            tokio::time::timeout(Duration::from_millis(50), c.wait_idle())
                .await
                .is_err(),
            "wait_idle must stay pending while a turn is in flight"
        );
        // Release the turn; the drain now completes.
        let _ = tx.send(());
        tokio::time::timeout(Duration::from_millis(500), c.wait_idle())
            .await
            .expect("wait_idle drains after the turn completes");
    }
}
