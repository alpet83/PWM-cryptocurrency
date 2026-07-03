//! Operator bridge federation controls (recover from sticky refusal without full restart).

use super::common::{ensure_operator_auth, ensure_ready};
use crate::transport::handshake_write_traced;
use crate::App;
use axum::extract::{ConnectInfo, State};
use axum::http::{HeaderMap, StatusCode};
use std::net::SocketAddr;

/// Clears local `bridge_federation_trust_refused` latch so the next successful peer hello
/// can restore federation paths. Does not mutate chain state.
pub(super) async fn v1_bridge_federation_reset(
    State(a): State<App>,
    conn: Option<ConnectInfo<SocketAddr>>,
    headers: HeaderMap,
) -> Result<StatusCode, (StatusCode, String)> {
    ensure_operator_auth(&a, conn.map(|v| v.0), &headers)?;
    ensure_ready(&a).await?;
    let mut hs = handshake_write_traced(&a, "api_handlers_bridge").await;
    hs.bridge_trust.refused = false;
    hs.bridge_trust.refusal_reason = None;
    Ok(StatusCode::NO_CONTENT)
}
