//! Post-handshake seed wire session: initial exchange, then steady heartbeat loop.

mod initial_exchange;
mod steady_session;

use super::super::super::*;
use crate::handshake::NodeHello;

/// State after the first `read_wire_msg` following cross-shard and account view sends.
pub(super) struct PostInitialExchange {
    pub close_reason: Option<PeerCloseReason>,
    pub close_detail: String,
}

pub(super) enum InitialExchangeOutcome {
    /// Send path failed; reconnect bookkeeping and retry sleep already applied.
    Aborted,
    Continue(PostInitialExchange),
}

pub(super) async fn seed_run_connected_session(
    app: &App,
    cfg: &TransportConfig,
    seed: std::net::SocketAddr,
    seed_key: &str,
    now_ms: u64,
    stream: &mut tokio::net::TcpStream,
    remote: NodeHello,
    drain_timeout_cap_ms: u64,
) {
    match initial_exchange::run_seed_initial_exchange(
        app, cfg, seed, seed_key, now_ms, stream, &remote,
    )
    .await
    {
        InitialExchangeOutcome::Aborted => {}
        InitialExchangeOutcome::Continue(state) => {
            steady_session::run_seed_steady_session(
                app,
                cfg,
                seed,
                seed_key,
                now_ms,
                stream,
                remote,
                drain_timeout_cap_ms,
                state,
            )
            .await;
        }
    }
}
