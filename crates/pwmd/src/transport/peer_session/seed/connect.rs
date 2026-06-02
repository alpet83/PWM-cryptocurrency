//! Seed outbound TCP connect with sticky trusted-session skip logic.

use super::super::super::*;
use super::super::{handshake_write_traced, has_sticky_trusted_session, sticky_session_window_ms};

pub(super) async fn seed_try_tcp_connect(
    app: &App,
    cfg: &TransportConfig,
    seed: std::net::SocketAddr,
    seed_key: &str,
    now_ms: u64,
) -> Option<tokio::net::TcpStream> {
    {
        let mut hs = handshake_write_traced(app, "seed_connect").await;
        let sticky_window_ms = sticky_session_window_ms(cfg);
        if has_sticky_trusted_session(&hs, seed_key, now_ms, sticky_window_ms) {
            record_reconnect(
                &mut hs,
                now_ms,
                seed_key,
                PeerReconnectReason::HealthySessionSkip,
                None,
            );
            let next_due = now_ms.saturating_add(cfg.heartbeat_interval_ms.max(200));
            set_seed_due(&mut hs, seed_key, next_due);
            drop(hs);
            tokio::time::sleep(std::time::Duration::from_millis(
                cfg.heartbeat_interval_ms.max(200),
            ))
            .await;
            return None;
        }
        record_reconnect(
            &mut hs,
            now_ms,
            seed_key,
            PeerReconnectReason::RetryAfterClose,
            None,
        );
    }
    info!(
        target: "pwmd::peer",
        "peer tcp connect started seed={} remote={}",
        seed_key,
        seed
    );
    let connect = tokio::time::timeout(
        std::time::Duration::from_millis(cfg.connect_timeout_ms.max(1)),
        tokio::net::TcpStream::connect(seed),
    )
    .await;
    match connect {
        Ok(Ok(v)) => {
            info!(
                target: "pwmd::peer",
                "peer tcp connect succeeded seed={} local={:?} remote={}",
                seed_key,
                v.local_addr().ok(),
                seed
            );
            Some(v)
        }
        Ok(Err(e)) => {
            let mut hs = handshake_write_traced(app, "seed_connect").await;
            warn!(
                target: "pwmd::peer",
                "peer tcp connect failed seed={} remote={} error={}",
                seed_key, seed, e
            );
            set_peer_error(&mut hs, now_ms, format!("seed {seed} connect_failed: {e}"));
            record_reconnect(
                &mut hs,
                now_ms,
                seed_key,
                PeerReconnectReason::ConnectFailure,
                Some("tcp_connect_failed"),
            );
            hs.transport.snapshot.session_retrying_total = hs
                .transport
                .snapshot
                .session_retrying_total
                .saturating_add(1);
            drop(hs);
            tokio::time::sleep(std::time::Duration::from_millis(
                super::super::peer_retry_sleep_ms(cfg, seed_key, now_ms),
            ))
            .await;
            None
        }
        Err(_) => {
            let mut hs = handshake_write_traced(app, "seed_connect").await;
            warn!(
                target: "pwmd::peer",
                "peer tcp connect timeout seed={} remote={} timeout_ms={}",
                seed_key,
                seed,
                cfg.connect_timeout_ms.max(1)
            );
            set_peer_error(&mut hs, now_ms, format!("seed {seed} connect_timeout"));
            record_reconnect(
                &mut hs,
                now_ms,
                seed_key,
                PeerReconnectReason::ConnectFailure,
                Some("tcp_connect_timeout"),
            );
            hs.transport.snapshot.session_retrying_total = hs
                .transport
                .snapshot
                .session_retrying_total
                .saturating_add(1);
            drop(hs);
            tokio::time::sleep(std::time::Duration::from_millis(
                super::super::peer_retry_sleep_ms(cfg, seed_key, now_ms),
            ))
            .await;
            None
        }
    }
}
