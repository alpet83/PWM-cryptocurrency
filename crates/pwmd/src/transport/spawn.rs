//! Long-running Tokio tasks for transport scheduling and peer I/O.

use tracing::{info, warn};

use crate::transport::peer_session::handshake_write_traced;

use super::{
    current_time_ms, process_inbound_socket, run_real_transport_tick, run_seed_session,
    run_transport_tick, App, TransportConfig,
};

/// Spawn a minimal transport scheduling loop (policy->dial/backoff wiring, no sockets).
pub fn spawn_transport_loop(app: App) {
    tokio::spawn(async move {
        let mut iv = tokio::time::interval(std::time::Duration::from_millis(500));
        loop {
            iv.tick().await;
            if !app.init.read().await.is_ready() {
                continue;
            }
            let now_ms = match current_time_ms() {
                Ok(v) => v,
                Err((_, e)) => {
                    warn!(target: "pwmd::peer", "transport tick skipped: {}", e);
                    continue;
                }
            };
            let mut hs = handshake_write_traced(&app, "transport_spawn").await;
            run_transport_tick(&mut hs, now_ms);
        }
    });
}

/// Spawn a minimal real socket transport loop (seed connect + NodeHello handshake).
pub fn spawn_real_transport_loop(app: App, cfg: TransportConfig) {
    tokio::spawn(async move {
        let tick_ms = cfg.retry_base_ms.max(100);
        let mut iv = tokio::time::interval(std::time::Duration::from_millis(tick_ms));
        loop {
            iv.tick().await;
            if !app.init.read().await.is_ready() {
                continue;
            }
            let now_ms = match current_time_ms() {
                Ok(v) => v,
                Err((_, e)) => {
                    warn!(target: "pwmd::peer", "real transport tick skipped: {}", e);
                    continue;
                }
            };
            run_real_transport_tick(&app, &cfg, now_ms).await;
        }
    });
}

pub fn spawn_peer_listener_loop(app: App, cfg: TransportConfig) {
    tokio::spawn(async move {
        let listener = match tokio::net::TcpListener::bind(cfg.peer_listen).await {
            Ok(v) => v,
            Err(e) => {
                warn!(
                    target: "pwmd::peer",
                    "peer listener bind {} failed: {}",
                    cfg.peer_listen,
                    e
                );
                warn!(
                    "peer listener bind {} failed: {} (Windows 10013: check reserved TCP ranges, e.g. netsh interface ipv4 show excludedportrange protocol=tcp; use other --transport-peer-listen ports)",
                    cfg.peer_listen,
                    e
                );
                return;
            }
        };
        info!(target: "pwmd::peer", "peer listener active at {}", cfg.peer_listen);
        loop {
            let (stream, peer) = match listener.accept().await {
                Ok(v) => v,
                Err(e) => {
                    warn!(target: "pwmd::peer", "peer listener accept failed: {}", e);
                    continue;
                }
            };
            let app = app.clone();
            let cfg = cfg.clone();
            tokio::spawn(async move {
                process_inbound_socket(&app, &cfg, stream, peer).await;
            });
        }
    });
}

pub fn spawn_stateful_transport_loop(app: App, cfg: TransportConfig) {
    tokio::spawn(async move {
        for seed in cfg.peer_seeds.clone() {
            let app = app.clone();
            let cfg = cfg.clone();
            tokio::spawn(async move {
                run_seed_session(app, cfg, seed).await;
            });
        }
    });
}
