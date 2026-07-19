//! Ready-made shutdown signals for [`serve`](crate::serve).
//!
//! A shutdown signal is simply a `Future` that resolves when the server should
//! stop accepting new connections and begin draining.

/// Complete when the process receives a shutdown signal.
///
/// On Unix this resolves on **either** `Ctrl+C` (`SIGINT`) **or** `SIGTERM`
/// (the signal orchestrators such as Docker/Kubernetes send). On other
/// platforms it resolves on `Ctrl+C`.
///
/// Installing the signal handler can fail (for example in a sandbox); if it
/// does, this falls back to `Ctrl+C` only rather than panicking.
///
/// # Examples
///
/// ```no_run
/// # async fn run() {
/// let shutdown = oxide_web_core::shutdown::ctrl_c();
/// // pass `shutdown` to `serve(..)`
/// shutdown.await;
/// # }
/// ```
pub async fn ctrl_c() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};

        let ctrl_c = tokio::signal::ctrl_c();
        match signal(SignalKind::terminate()) {
            Ok(mut term) => {
                tokio::select! {
                    _ = ctrl_c => {}
                    _ = term.recv() => {}
                }
            }
            Err(err) => {
                tracing::warn!(error = %err, "could not install SIGTERM handler; using Ctrl+C only");
                let _ = ctrl_c.await;
            }
        }
    }

    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}

/// A signal that never fires.
///
/// Useful in tests or long-running servers that are shut down by dropping the
/// task rather than by a signal.
pub async fn never() {
    std::future::pending::<()>().await
}
