//! Post-connect cross-shard fact fan-out before steady peer sessions.

use super::super::super::*;
use super::super::super::{
    handshake_write_traced, mark_trusted_peer_live, merge_account_views, merge_cross_shard_facts,
    peer_sync_v1, read_wire_msg, route_cluster_stub, route_sync_stub, send_account_views,
    send_cluster_prop, send_cross_shard_facts, sync_live, write_wire_msg, PeerWireMsg,
};
use super::{InitialExchangeOutcome, PostInitialExchange};
use crate::block_timing;
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
    let mut wait_logged = false;
    // Guard against initial sync racing with snapshot restore.
    while !app.init.read().await.is_ready() {
        if !wait_logged {
            info!(
                target: "pwmd::peer",
                "peer session waiting for init ready seed=outbound node_id={}",
                remote.node.node_id
            );
            wait_logged = true;
        }
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }
    if let Err(err) = send_cross_shard_facts(&app, &cfg, stream).await {
        let mut hs = handshake_write_traced(app, "seed_initial_exchange").await;
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
        let mut hs = handshake_write_traced(app, "seed_initial_exchange").await;
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
    if let Err(err) = send_cluster_prop(&app, &cfg, stream, remote).await {
        let mut hs = handshake_write_traced(app, "seed_initial_exchange").await;
        let close_reason = wire_close_reason(&err);
        set_peer_error(
            &mut hs,
            now_ms,
            format!("seed {seed} cluster_propose_write_failed: {err}"),
        );
        record_peer_close(
            &mut hs,
            now_ms,
            seed_key,
            Some(&remote.node.node_id),
            close_reason,
            Some(detail_with_err("cluster_propose_write_failed", &err).as_str()),
        );
        record_reconnect(
            &mut hs,
            now_ms,
            seed_key,
            reconnect_from_close(close_reason),
            Some(detail_with_err("cluster_propose_write_failed", &err).as_str()),
        );
        tokio::time::sleep(std::time::Duration::from_millis(
            super::super::super::peer_retry_sleep_ms(cfg, seed_key, now_ms),
        ))
        .await;
        return InitialExchangeOutcome::Aborted;
    }
    let mut close_reason = None;
    let mut close_detail = "explicit_shutdown".to_string();
    let sync_v1 = peer_sync_v1(remote);
    let same_shard = remote.cluster.domain_hi == app.identity.cluster_domain_hi;
    let (sync_hdr_cap, sync_blk_cap) = sync_live::sync_caps(remote);
    let can_cup = remote
        .capabilities
        .sync_profile
        .as_ref()
        .map(|x| x.supports_epoch_catchup)
        .unwrap_or(false);
    let mut sync_seq_no = 0u64;
    match read_wire_msg(stream, cfg.heartbeat_timeout_ms).await {
        Ok(PeerWireMsg::CrossShardFacts { facts }) => {
            merge_cross_shard_facts(&app, facts, true).await;
            let mut hs = handshake_write_traced(app, "seed_initial_exchange").await;
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
            let mut hs = handshake_write_traced(app, "seed_initial_exchange").await;
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
            ..
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
                let mut hs = handshake_write_traced(app, "seed_initial_exchange").await;
                close_reason = Some(wire_close_reason(&err));
                close_detail = detail_with_err("heartbeat_ack_write_failed", &err);
                set_peer_error(
                    &mut hs,
                    now_ms,
                    format!("seed {seed} wire_heartbeat_ack_write_failed: {err}"),
                );
            } else {
                let mut hs = handshake_write_traced(app, "seed_initial_exchange").await;
                mark_trusted_peer_live(
                    &mut hs,
                    &remote.node.node_id,
                    current_time_ms().unwrap_or(now_ms),
                );
            }
        }
        Ok(msg @ (PeerWireMsg::ClusterPropose { .. } | PeerWireMsg::ClusterAttest { .. })) => {
            let route_cluster_start_ms = current_time_ms().unwrap_or(now_ms);
            let maybe_attest = route_cluster_stub(app, &remote.node.node_id, msg).await;
            let route_cluster_done_ms = current_time_ms().unwrap_or(route_cluster_start_ms);
            let route_cluster_latency_ms =
                route_cluster_done_ms.saturating_sub(route_cluster_start_ms);
            if route_cluster_latency_ms >= 100 {
                warn!(
                    target: "pwmd::peer",
                    "seed initial_exchange cluster_route_slow seed={} node_id={} ts_ms={} latency_ms={}",
                    seed,
                    remote.node.node_id,
                    route_cluster_done_ms,
                    route_cluster_latency_ms,
                );
            }
            if let Some(attest) = maybe_attest {
                let h = attest.height;
                let r = attest.round;
                if let Err(err) = write_wire_msg(
                    stream,
                    &PeerWireMsg::ClusterAttest { msg: attest },
                    cfg.heartbeat_timeout_ms,
                )
                .await
                {
                    let mut hs = handshake_write_traced(app, "seed_initial_exchange").await;
                    close_reason = Some(wire_close_reason(&err));
                    close_detail = detail_with_err("cluster_attest_write_failed", &err);
                    set_peer_error(
                        &mut hs,
                        now_ms,
                        format!("seed {seed} cluster_attest_write_failed: {err}"),
                    );
                } else if let Some(bt) = app.block_timing.as_ref() {
                    block_timing::note_att_wire(
                        bt,
                        block_timing::AttCtx {
                            h,
                            r,
                            t_ms: block_timing::now_ms_f64(),
                            att_id: app.node_instance_id.clone(),
                        },
                    );
                }
            }
        }
        Ok(
            msg @ (PeerWireMsg::SyncProfileAnnounce { .. }
            | PeerWireMsg::SyncTipAnnounce { .. }
            | PeerWireMsg::SyncHeadersReq { .. }
            | PeerWireMsg::SyncHeadersBatch { .. }
            | PeerWireMsg::SyncBlocksReq { .. }
            | PeerWireMsg::SyncBlocksBatch { .. }
            | PeerWireMsg::SyncTxAnnounce { .. }
            | PeerWireMsg::SyncTxReq { .. }
            | PeerWireMsg::SyncTxBatch { .. }
            | PeerWireMsg::SyncNack { .. }
            | PeerWireMsg::SyncCatchupReq { .. }
            | PeerWireMsg::SyncCatchupChunk { .. }
            | PeerWireMsg::SyncCatchupDone { .. }),
        ) => {
            let outcome = route_sync_stub(
                app,
                cfg,
                stream,
                Some(seed_key),
                &remote.node.node_id,
                msg,
                sync_v1,
                app.identity.cluster_domain_hi,
                same_shard,
                sync_hdr_cap,
                sync_blk_cap,
                can_cup,
                &mut sync_seq_no,
            )
            .await;
            if let super::super::super::SyncRouteOutcome::Disconnect { reason, detail } = outcome {
                close_reason = Some(reason);
                close_detail = detail;
            }
        }
        Ok(_) => {}
        Err(err) => {
            if !is_wire_timeout(&err) {
                let mut hs = handshake_write_traced(app, "seed_initial_exchange").await;
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
