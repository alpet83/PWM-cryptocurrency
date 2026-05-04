//! Post-connect cross-shard fact fan-out before steady peer sessions.

use super::super::super::*;
use super::super::super::{
    mark_trusted_peer_live, merge_account_views, merge_cross_shard_facts, read_wire_msg,
    send_account_views, send_cross_shard_facts, write_wire_msg, PeerWireMsg,
};
use super::{InitialExchangeOutcome, PostInitialExchange};
use crate::handshake::NodeHello;

pub(super) async fn run_seed_initial_exchange(
    app: &App,
    cfg: &TransportConfig,
    seed: std::net::SocketAddr,
    seed_key: &str,
    now_ms: u64,
    stream: &mut tokio::net::TcpStream,
    remote: &NodeHello,
) -> InitialExchangeOutcome {
    if let Err(err) = send_cross_shard_facts(&app, &cfg, stream).await {
        let mut hs = app.handshake.write().await;
        let close_reason = wire_close_reason(&err);
        set_peer_error(
            &mut hs,
            now_ms,
            format!("seed {seed} cross_shard_facts_write_failed: {err}"),
        );
        record_peer_close(
            &mut hs,
            now_ms,
            seed_key,
            Some(&remote.node.node_id),
            close_reason,
            Some(detail_with_err("cross_shard_facts_write_failed", &err).as_str()),
        );
        record_reconnect(
            &mut hs,
            now_ms,
            seed_key,
            reconnect_from_close(close_reason),
            Some(detail_with_err("cross_shard_facts_write_failed", &err).as_str()),
        );
        tokio::time::sleep(std::time::Duration::from_millis(
            super::super::super::peer_retry_sleep_ms(cfg, seed_key, now_ms),
        ))
        .await;
        return InitialExchangeOutcome::Aborted;
    }
    if let Err(err) = send_account_views(&app, &cfg, stream).await {
        let mut hs = app.handshake.write().await;
        let close_reason = wire_close_reason(&err);
        set_peer_error(
            &mut hs,
            now_ms,
            format!("seed {seed} account_views_write_failed: {err}"),
        );
        record_peer_close(
            &mut hs,
            now_ms,
            seed_key,
            Some(&remote.node.node_id),
            close_reason,
            Some(detail_with_err("account_views_write_failed", &err).as_str()),
        );
        record_reconnect(
            &mut hs,
            now_ms,
            seed_key,
            reconnect_from_close(close_reason),
            Some(detail_with_err("account_views_write_failed", &err).as_str()),
        );
        tokio::time::sleep(std::time::Duration::from_millis(
            super::super::super::peer_retry_sleep_ms(cfg, seed_key, now_ms),
        ))
        .await;
        return InitialExchangeOutcome::Aborted;
    }
    let mut close_reason = None;
    let mut close_detail = "explicit_shutdown".to_string();
    match read_wire_msg(stream, cfg.heartbeat_timeout_ms).await {
        Ok(PeerWireMsg::CrossShardFacts { facts }) => {
            merge_cross_shard_facts(&app, facts, true).await;
            let mut hs = app.handshake.write().await;
            mark_trusted_peer_live(
                &mut hs,
                &remote.node.node_id,
                current_time_ms().unwrap_or(now_ms),
            );
        }
        Ok(PeerWireMsg::AccountViews { rows }) => {
            merge_account_views(
                &app,
                rows,
                true,
                &remote.node.node_id,
                remote.cluster.domain_hi,
                now_ms,
            )
            .await;
            let mut hs = app.handshake.write().await;
            mark_trusted_peer_live(
                &mut hs,
                &remote.node.node_id,
                current_time_ms().unwrap_or(now_ms),
            );
        }
        Ok(PeerWireMsg::Heartbeat {
            unix_ms,
            chain_tip_height,
            federation_shard_id,
            federation_gossip,
        }) => {
            crate::federation::merge_remote_hb(
                &app,
                true,
                &remote,
                &remote.node.node_id,
                unix_ms,
                chain_tip_height,
                federation_shard_id,
                federation_gossip,
            )
            .await;
            if let Err(err) = write_wire_msg(
                stream,
                &PeerWireMsg::HeartbeatAck { unix_ms },
                cfg.heartbeat_timeout_ms,
            )
            .await
            {
                let mut hs = app.handshake.write().await;
                close_reason = Some(wire_close_reason(&err));
                close_detail = detail_with_err("heartbeat_ack_write_failed", &err);
                set_peer_error(
                    &mut hs,
                    now_ms,
                    format!("seed {seed} wire_heartbeat_ack_write_failed: {err}"),
                );
            } else {
                let mut hs = app.handshake.write().await;
                mark_trusted_peer_live(
                    &mut hs,
                    &remote.node.node_id,
                    current_time_ms().unwrap_or(now_ms),
                );
            }
        }
        Ok(_) => {}
        Err(err) => {
            if !is_wire_timeout(&err) {
                let mut hs = app.handshake.write().await;
                close_reason = Some(wire_close_reason(&err));
                close_detail = detail_with_err("initial_read_failed", &err);
                set_peer_error(
                    &mut hs,
                    now_ms,
                    format!("seed {seed} cross_shard_facts_read_failed: {err}"),
                );
            }
        }
    }
    InitialExchangeOutcome::Continue(PostInitialExchange {
        close_reason,
        close_detail,
    })
}
