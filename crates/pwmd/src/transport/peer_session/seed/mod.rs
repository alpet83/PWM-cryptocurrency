//! Outbound seed dial session: TCP connect, handshake, then steady messaging loop.

use super::super::*;

mod connect;
mod handshake;
mod session;

async fn seed_wait_ms(app: &App, cfg: &TransportConfig, seed_key: &str, now_ms: u64) -> u64 {
    let marker_wait = {
        let hs = crate::transport::handshake_read_traced(app, "seed_mod").await;
        hs.transport
            .seed_peers
            .get(seed_key)
            .map(|x| x.next_due_ms.saturating_sub(now_ms))
            .unwrap_or(0)
    };
    marker_wait.max(super::peer_retry_sleep_ms(cfg, seed_key, now_ms))
}

pub(crate) async fn run_seed_session(app: App, cfg: TransportConfig, seed: std::net::SocketAddr) {
    const DRAIN_TIMEOUT_CAP_MS: u64 = 25;
    let seed_key = seed.to_string();
    loop {
        if !app.init.read().await.is_ready() {
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            continue;
        }
        let now_ms = current_time_ms().unwrap_or(0);
        let mut stream =
            match connect::seed_try_tcp_connect(&app, &cfg, seed, &seed_key, now_ms).await {
                Some(s) => s,
                None => continue,
            };
        let remote = match handshake::seed_finish_handshake(
            &app,
            &cfg,
            seed,
            &seed_key,
            now_ms,
            &mut stream,
        )
        .await
        {
            Some(r) => r,
            None => continue,
        };
        session::seed_run_connected_session(
            &app,
            &cfg,
            seed,
            &seed_key,
            now_ms,
            &mut stream,
            remote,
            DRAIN_TIMEOUT_CAP_MS,
        )
        .await;
        let wait_ms =
            seed_wait_ms(&app, &cfg, &seed_key, current_time_ms().unwrap_or(now_ms)).await;
        tokio::time::sleep(std::time::Duration::from_millis(wait_ms)).await;
    }
}
