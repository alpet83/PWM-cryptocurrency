//! Graceful node shutdown: persist snapshot then stop HTTP server.

use crate::App;
use axum::extract::State;
use axum::http::StatusCode;
use tracing::info;

pub(super) async fn v1_shutdown(
    State(app): State<App>,
) -> Result<StatusCode, (StatusCode, String)> {
    {
        let inner = app.inner.read().await;
        if let Some(ref backend) = app.autosnapshot_backend {
            backend.save_seal_persist(&inner).map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("shutdown snapshot persist failed: {e}"),
                )
            })?;
        }
    }
    if let Ok(mut slot) = app.shutdown_tx.lock() {
        if let Some(tx) = slot.take() {
            let _ = tx.send(());
        }
    }
    info!("graceful shutdown requested via POST /v1/shutdown");
    Ok(StatusCode::NO_CONTENT)
}
