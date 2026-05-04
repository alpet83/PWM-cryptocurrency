//! Operator bridge federation controls (recover from sticky refusal without full restart).

use super::common::ensure_ready;
use crate::App;
use axum::extract::State;
use axum::http::StatusCode;

/// Clears local `bridge_federation_trust_refused` latch so the next successful peer hello
/// can restore federation paths. Does not mutate chain state.
pub(super) async fn v1_bridge_federation_reset(
    State(a): State<App>,
) -> Result<StatusCode, (StatusCode, String)> {
    ensure_ready(&a).await?;
    let mut hs = a.handshake.write().await;
    hs.bridge_trust.refused = false;
    hs.bridge_trust.refusal_reason = None;
    Ok(StatusCode::NO_CONTENT)
}
