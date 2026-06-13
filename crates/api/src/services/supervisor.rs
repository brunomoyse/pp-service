//! Background-service supervision: keep a long-lived task alive across panics
//! and stop it cleanly on shutdown. Lives in the library (not main.rs) so the
//! restart behavior is testable.

/// Spawn a long-lived background service under supervision: if its task exits or
/// panics it is restarted (after a short backoff); when `shutdown` flips, the
/// task is aborted and the supervisor returns.
pub fn supervise(
    name: &'static str,
    mut shutdown: tokio::sync::watch::Receiver<bool>,
    spawn: impl Fn() -> tokio::task::JoinHandle<()> + Send + 'static,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            let mut handle = spawn();
            tokio::select! {
                _ = shutdown.changed() => {
                    handle.abort();
                    tracing::info!("{name}: stopping on shutdown");
                    return;
                }
                res = &mut handle => {
                    if *shutdown.borrow() {
                        return;
                    }
                    match res {
                        Ok(()) => {
                            tracing::error!("{name}: task exited unexpectedly; restarting in 5s")
                        }
                        Err(e) => {
                            tracing::error!("{name}: task panicked ({e}); restarting in 5s")
                        }
                    }
                    tokio::select! {
                        _ = shutdown.changed() => return,
                        _ = tokio::time::sleep(std::time::Duration::from_secs(5)) => {}
                    }
                }
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::supervise;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::sync::watch;

    // `start_paused` auto-advances tokio's clock whenever the runtime is idle, so
    // the supervisor's 5s restart backoff elapses instantly instead of for real.
    #[tokio::test(start_paused = true)]
    async fn restarts_a_panicking_task_then_stops_on_shutdown() {
        let spawns = Arc::new(AtomicUsize::new(0));
        let (tx, rx) = watch::channel(false);

        let handle = supervise("test_service", rx, {
            let spawns = spawns.clone();
            move || {
                let attempt = spawns.fetch_add(1, Ordering::SeqCst);
                tokio::spawn(async move {
                    if attempt == 0 {
                        panic!("boom on first run");
                    }
                    // Subsequent runs stay alive until aborted.
                    std::future::pending::<()>().await;
                })
            }
        });

        // Let the panic propagate and the (auto-advanced) backoff elapse.
        tokio::time::sleep(Duration::from_secs(6)).await;
        assert!(
            spawns.load(Ordering::SeqCst) >= 2,
            "a panicking task must be restarted (spawned at least twice)"
        );

        // Flipping the shutdown channel makes the supervisor abort and return.
        tx.send(true).unwrap();
        tokio::time::sleep(Duration::from_secs(1)).await;
        assert!(
            handle.await.is_ok(),
            "supervisor returns cleanly after shutdown"
        );
    }
}
