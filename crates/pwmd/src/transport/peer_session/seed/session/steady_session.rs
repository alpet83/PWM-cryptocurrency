//! Heartbeat-driven steady peer loop after initial handshake merge.

use super::super::super::*;
use super::super::super::{
    mark_trusted_peer_live, merge_account_views, merge_cross_shard_facts, peer_heartbeat_wire,
    peer_sync_v1, read_wire_msg, route_cluster_stub, route_sync_stub, send_account_views,
    send_cluster_prop, send_cross_shard_facts, send_sync_tx_batch, sticky_session_window_ms,
    sync_live, sync_mode_text, write_wire_msg, PeerWireMsg,
};
use super::PostInitialExchange;
use crate::handshake::NodeHello;

pub(super) async fn run_seed_steady_session(
    app: &App,
    cfg: &TransportConfig,
    seed: std::net::SocketAddr,
    seed_key: &str,
    now_ms: u64,
    stream: &mut tokio::net::TcpStream,
    remote: NodeHello,
    drain_timeout_cap_ms: u64,
    post: PostInitialExchange,
) {
    let mut close_reason = post.close_reason;
    let mut close_detail = post.close_detail;
    let hb = cfg.heartbeat_interval_ms.max(200);
    let mut live = close_reason.is_none();
    let sync_v1 = peer_sync_v1(&remote);
    let same_shard = remote.cluster.domain_hi == app.identity.cluster_domain_hi;
    let (sync_hdr_cap, sync_blk_cap) = sync_live::sync_caps(&remote);
    let can_cup = remote
        .capabilities
        .sync_profile
        .as_ref()
        .map(|x| x.supports_epoch_catchup)
        .unwrap_or(false);
    let mut sync_seq_no = 0u64;
    info!(
        target: "pwmd::peer",
        "peer sync mode negotiated seed={} node_id={} mode={}",
        seed,
        remote.node.node_id,
        sync_mode_text(&remote)
    );
    while live {
        tokio::time::sleep(std::time::Duration::from_millis(hb)).await;
        let ts = current_time_ms().unwrap_or(0);
        let hb_out = peer_heartbeat_wire(&app, ts).await;
        if let Err(err) = write_wire_msg(stream, &hb_out, cfg.heartbeat_timeout_ms).await {
            let mut hs = app.handshake.write().await;
            close_reason = Some(wire_close_reason(&err));
            close_detail = detail_with_err("heartbeat_write_failed", &err);
            set_peer_error(
                &mut hs,
                ts,
                format!("seed {seed} wire_heartbeat_write_failed: {err}"),
            );
            break;
        }
        if let Err(err) = send_cross_shard_facts(&app, &cfg, stream).await {
            let mut hs = app.handshake.write().await;
            close_reason = Some(wire_close_reason(&err));
            close_detail = detail_with_err("cross_shard_facts_write_failed", &err);
            set_peer_error(
                &mut hs,
                ts,
                format!("seed {seed} cross_shard_facts_write_failed: {err}"),
            );
            break;
        }
        if let Err(err) = send_account_views(&app, &cfg, stream).await {
            let mut hs = app.handshake.write().await;
            close_reason = Some(wire_close_reason(&err));
            close_detail = detail_with_err("account_views_write_failed", &err);
            set_peer_error(
                &mut hs,
                ts,
                format!("seed {seed} account_views_write_failed: {err}"),
            );
            break;
        }
        if let Err(err) = send_sync_tx_batch(&app, &cfg, stream, &remote, &mut sync_seq_no).await {
            let mut hs = app.handshake.write().await;
            close_reason = Some(wire_close_reason(&err));
            close_detail = detail_with_err("sync_tx_batch_write_failed", &err);
            set_peer_error(
                &mut hs,
                ts,
                format!("seed {seed} sync_tx_batch_write_failed: {err}"),
            );
            break;
        }
        if let Err(err) = send_cluster_prop(&app, &cfg, stream, &remote).await {
            let mut hs = app.handshake.write().await;
            close_reason = Some(wire_close_reason(&err));
            close_detail = detail_with_err("cluster_propose_write_failed", &err);
            set_peer_error(
                &mut hs,
                ts,
                format!("seed {seed} cluster_propose_write_failed: {err}"),
            );
            break;
        }
        if let Err(err) =
            sync_live::send_sync_tip(&app, &cfg, stream, &remote, &mut sync_seq_no).await
        {
            let mut hs = app.handshake.write().await;
            close_reason = Some(wire_close_reason(&err));
            close_detail = detail_with_err("sync_tip_write_failed", &err);
            set_peer_error(
                &mut hs,
                ts,
                format!("seed {seed} sync_tip_write_failed: {err}"),
            );
            break;
        }
        let mut heartbeat_acked = false;
        let mut read_timeout_ms = cfg.heartbeat_timeout_ms;
        while live {
            match read_wire_msg(stream, read_timeout_ms).await {
                Ok(PeerWireMsg::HeartbeatAck { .. }) => {
                    heartbeat_acked = true;
                    let mut hs = app.handshake.write().await;
                    mark_trusted_peer_live(&mut hs, &remote.node.node_id, ts);
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
                        let mut hs = app.handshake.write().await;
                        close_reason = Some(wire_close_reason(&err));
                        close_detail = detail_with_err("heartbeat_ack_write_failed", &err);
                        set_peer_error(
                            &mut hs,
                            ts,
                            format!("seed {seed} wire_heartbeat_ack_write_failed: {err}"),
                        );
                        live = false;
                    } else {
                        let mut hs = app.handshake.write().await;
                        mark_trusted_peer_live(&mut hs, &remote.node.node_id, ts);
                    }
                }
                Ok(PeerWireMsg::CrossShardFacts { facts }) => {
                    merge_cross_shard_facts(&app, facts, true).await;
                    let mut hs = app.handshake.write().await;
                    mark_trusted_peer_live(&mut hs, &remote.node.node_id, ts);
                }
                Ok(PeerWireMsg::AccountViews { rows }) => {
                    merge_account_views(
                        &app,
                        rows,
                        true,
                        &remote.node.node_id,
                        remote.cluster.domain_hi,
                        ts,
                    )
                    .await;
                    let mut hs = app.handshake.write().await;
                    mark_trusted_peer_live(&mut hs, &remote.node.node_id, ts);
                }
                Ok(
                    msg @ (PeerWireMsg::ClusterPropose { .. } | PeerWireMsg::ClusterAttest { .. }),
                ) => {
                    let maybe_attest = route_cluster_stub(app, &remote.node.node_id, msg).await;
                    if let Some(attest) = maybe_attest {
                        if let Err(err) = write_wire_msg(
                            stream,
                            &PeerWireMsg::ClusterAttest { msg: attest },
                            cfg.heartbeat_timeout_ms,
                        )
                        .await
                        {
                            let mut hs = app.handshake.write().await;
                            close_reason = Some(wire_close_reason(&err));
                            close_detail = detail_with_err("cluster_attest_write_failed", &err);
                            set_peer_error(
                                &mut hs,
                                ts,
                                format!("seed {seed} cluster_attest_write_failed: {err}"),
                            );
                            live = false;
                            continue;
                        }
                    }
                    let mut hs = app.handshake.write().await;
                    mark_trusted_peer_live(&mut hs, &remote.node.node_id, ts);
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
                    match outcome {
                        super::super::super::SyncRouteOutcome::Continue => {
                            let mut hs = app.handshake.write().await;
                            mark_trusted_peer_live(&mut hs, &remote.node.node_id, ts);
                        }
                        super::super::super::SyncRouteOutcome::Disconnect { reason, detail } => {
                            close_reason = Some(reason);
                            close_detail = detail;
                            live = false;
                        }
                    }
                }
                Ok(other) => {
                    let mut hs = app.handshake.write().await;
                    close_reason = Some(PeerCloseReason::ProtocolError);
                    close_detail = format!("heartbeat_unexpected: {other:?}");
                    set_peer_error(
                        &mut hs,
                        ts,
                        format!("seed {seed} wire_heartbeat_unexpected: {other:?}"),
                    );
                    live = false;
                }
                Err(err) => {
                    if is_wire_timeout(&err) {
                        break;
                    }
                    let mut hs = app.handshake.write().await;
                    let reason = wire_close_reason(&err);
                    close_reason = Some(reason);
                    close_detail = detail_with_err("heartbeat_read_failed", &err);
                    set_peer_error(
                        &mut hs,
                        ts,
                        format!("seed {seed} wire_heartbeat_read_failed: {err}"),
                    );
                    live = false;
                }
            }
            if !live || !heartbeat_acked {
                continue;
            }
            read_timeout_ms = cfg.heartbeat_timeout_ms.min(drain_timeout_cap_ms).max(1);
        }
    }
    {
        let mut hs = app.handshake.write().await;
        let reason = close_reason.unwrap_or(PeerCloseReason::ExplicitShutdown);
        record_peer_close(
            &mut hs,
            current_time_ms().unwrap_or(now_ms),
            seed_key,
            Some(&remote.node.node_id),
            reason,
            Some(close_detail.as_str()),
        );
        record_reconnect(
            &mut hs,
            current_time_ms().unwrap_or(now_ms),
            seed_key,
            reconnect_from_close(reason),
            Some(close_detail.as_str()),
        );
        let sticky_window_ms = sticky_session_window_ms(cfg);
        let disconnect_ts = current_time_ms().unwrap_or(0);
        if let Some(last_node_id) = hs
            .transport
            .seed_peers
            .get(seed_key)
            .and_then(|s| s.last_node_id.clone())
        {
            if let Some(peer) = hs.peers.get_mut(&last_node_id) {
                let recently_alive =
                    disconnect_ts.saturating_sub(peer.last_seen_ms) <= sticky_window_ms;
                if !recently_alive {
                    peer.status = PeerStatus::Disconnected;
                }
            }
        }
    }
}
