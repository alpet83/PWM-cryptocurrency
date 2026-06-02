//! Graceful node shutdown: persist snapshot then stop HTTP server.

use crate::snapshot::SealPersistMode;
use crate::App;
use axum::extract::State;
use axum::http::StatusCode;
use std::sync::atomic::Ordering;
use tracing::{error, info};

#[derive(Clone, Copy, Debug)]
pub(crate) enum ShutdownReason {
    Rpc,
    Signal(&'static str),
    DebugStop,
}

impl ShutdownReason {
    fn key(self) -> &'static str {
        match self {
            ShutdownReason::Rpc => "rpc",
            ShutdownReason::Signal(kind) => kind,
            ShutdownReason::DebugStop => "debug_stop",
        }
    }
}

pub(crate) async fn graceful_shutdown_request(
    app: &App,
    reason: ShutdownReason,
) -> Result<(), String> {
    if app.shutdown_requested.swap(true, Ordering::SeqCst) {
        return Ok(());
    }

    let mut snapshot_err = None;
    {
        let inner = app.inner.read().await;
        if let Some(ref backend) = app.autosnapshot_backend {
            if let Err(err) = backend.save_seal_persist(&inner, SealPersistMode::ShutdownFull) {
                snapshot_err = Some(format!("shutdown snapshot persist failed: {err}"));
            }
        }
    }
    if let Ok(mut slot) = app.shutdown_tx.lock() {
        if let Some(tx) = slot.take() {
            let _ = tx.send(());
        }
    }
    info!(
        "#INFO: pwmd остановлено оператором reason={} node_id={}",
        reason.key(),
        app.node_instance_id
    );
    info!(
        "#INFO: pwmd stopped by operator reason={} node_id={}",
        reason.key(),
        app.node_instance_id
    );
    if let Some(err) = snapshot_err {
        error!("{}", err);
        return Err(err);
    }
    Ok(())
}

pub(super) async fn v1_shutdown(
    State(app): State<App>,
) -> Result<StatusCode, (StatusCode, String)> {
    graceful_shutdown_request(&app, ShutdownReason::Rpc)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::{graceful_shutdown_request, ShutdownReason};
    use crate::app_from_dev_net;
    use std::sync::atomic::Ordering;

    #[tokio::test]
    async fn shutdown_request_sets_guard() {
        let app = app_from_dev_net();
        let _ = graceful_shutdown_request(&app, ShutdownReason::DebugStop).await;
        assert!(app.shutdown_requested.load(Ordering::SeqCst));
    }
}
