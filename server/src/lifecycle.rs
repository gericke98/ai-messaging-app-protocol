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
    /// Retained so the channel is never receiver-less; this ensures `tx.subscribe()`
    /// called *after* `trigger()` produces a receiver whose `borrow()` returns `true`.
    _rx: Arc<watch::Receiver<bool>>,
    flag: Arc<AtomicBool>,
    tracker: TaskTracker,
}

impl ShutdownController {
    pub fn new() -> Self {
        let (tx, rx) = watch::channel(false);
        Self {
            tx: Arc::new(tx),
            _rx: Arc::new(rx),
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
        self.flag.store(true, Ordering::Relaxed);
        // Silenced: the AtomicBool is the authoritative state. `watch` persists the
        // last-sent value, so any receiver created *after* this call still observes
        // `true` via `borrow()`. A send error (no receivers yet) is therefore safe.
        let _ = self.tx.send(true);
    }

    /// Returns `true` after `trigger()` has been called at least once.
    pub fn is_shutting_down(&self) -> bool {
        self.flag.load(Ordering::Relaxed)
    }

    /// Spawn an in-flight agent turn as a tracked task. The drain (`wait_idle`)
    /// waits for every spawned turn to finish. Use this instead of `tokio::spawn`
    /// for agent turns so a turn already underway at shutdown still completes.
    pub fn spawn_turn<F>(&self, fut: F)
    where
        F: std::future::Future<Output = ()> + Send + 'static,
    {
        drop(self.tracker.spawn(fut)); // fire-and-forget: TaskTracker still counts the task
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

/// Spawn a task that triggers `controller` on the first SIGTERM or SIGINT.
///
/// Today the server only caught SIGINT (`ctrl_c`) — Docker/orchestrators send
/// SIGTERM, which had no handler, so the process was killed immediately. This
/// catches both. Unix only; on other targets it falls back to SIGINT alone
/// (the production target is Linux).
pub fn install_signals(controller: ShutdownController) {
    tokio::spawn(async move {
        #[cfg(unix)]
        {
            use tokio::signal::unix::{signal, SignalKind};
            let mut term = match signal(SignalKind::terminate()) {
                Ok(s) => s,
                Err(e) => {
                    tracing::error!("[lifecycle] failed to install SIGTERM handler: {e}");
                    return;
                }
            };
            tokio::select! {
                _ = term.recv() => tracing::info!("[lifecycle] received SIGTERM; beginning graceful shutdown"),
                _ = tokio::signal::ctrl_c() => tracing::info!("[lifecycle] received SIGINT; beginning graceful shutdown"),
            }
        }
        #[cfg(not(unix))]
        {
            let _ = tokio::signal::ctrl_c().await;
            tracing::info!("[lifecycle] received Ctrl-C; beginning graceful shutdown");
        }
        controller.trigger();
    });
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
    async fn wait_for_shutdown_already_triggered_before_subscribe() {
        let c = ShutdownController::new();
        c.trigger(); // trigger first
        let rx = c.subscribe(); // subscribe after
        tokio::time::timeout(Duration::from_millis(100), wait_for_shutdown(rx))
            .await
            .expect("already-triggered case resolves immediately");
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
