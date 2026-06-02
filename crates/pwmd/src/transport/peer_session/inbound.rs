//! Inbound TCP peer sessions: hello exchange and steady wire multiplexer.

use super::super::*;
use super::{
    handshake_write_traced, merge_cross_shard_facts, peer_sync_v1, read_wire_msg,
    route_cluster_stub, route_sync_stub, send_account_views, send_cluster_prop,
    send_cross_shard_facts, send_sync_tx_batch, sync_live, sync_mode_text, write_wire_msg,
    PeerWireMsg,
};

fn hello_sync_profile_text(hello: &crate::handshake::NodeHello) -> String {
    match hello.capabilities.sync_profile.as_ref() {
        Some(v) => format!(
            "sync_v={} hdr_cap={} blk_cap={} tx_cap={} cup={}",
            v.sync_wire_version,
            v.max_headers_per_msg,
            v.max_blocks_per_msg,
            v.max_txs_per_msg,
            v.supports_epoch_catchup
        ),
        None => "sync_profile=none".to_string(),
    }
}

fn hello_trace_fields(hello: &crate::handshake::NodeHello) -> String {
    format!(
        "network_id={} genesis_hash={:?} domain_hi=0x{:02X} cluster_id={} node_id={} node_instance_id={:?} protocol_version={} services={:?} tx_features={:?} {} deployment_profile={:?} seal_role={:?} cluster_role={:?} cluster_members={:?} quorum_k={:?} quorum_n={:?} nonce_len={} signature_len={} hello_ts_ms={} chain_tip_h={:?} federation_shard_id={:?} bridge_commitment={:?}",
        hello.network_id,
        hello.genesis_hash,
        hello.cluster.domain_hi,
        hello.cluster.cluster_id,
        hello.node.node_id,
        hello.capabilities.node_instance_id,
        hello.capabilities.protocol_version,
        hello.capabilities.services,
        hello.capabilities.tx_features,
        hello_sync_profile_text(hello),
        hello.capabilities.deployment_profile,
        hello.capabilities.seal_role,
        hello.capabilities.cluster_role,
        hello.capabilities.cluster_members,
        hello.capabilities.cluster_quorum_k,
        hello.capabilities.cluster_quorum_n,
        hello.nonce.len(),
        hello.signature.len(),
        hello.timestamp_ms,
        hello.chain_tip_height,
        hello.federation_shard_id,
        hello.bridge_commitment,
    )
}

fn sync_wire_reason(err: &str) -> Option<&'static str> {
    if err.contains("wire_decode_failed") {
        return Some("decode_failed");
    }
    if err.contains("wire_invalid_frame_len") {
        return Some("invalid_frame_len");
    }
    None
}

fn peer_wire_msg_kind(msg: &PeerWireMsg) -> &'static str {
    match msg {
        PeerWireMsg::Hello { .. } => "hello",
        PeerWireMsg::HelloAck { .. } => "hello_ack",
        PeerWireMsg::Heartbeat { .. } => "heartbeat",
        PeerWireMsg::HeartbeatAck { .. } => "heartbeat_ack",
        PeerWireMsg::CrossShardFacts { .. } => "cross_shard_facts",
        PeerWireMsg::AccountViews { .. } => "account_views",
        PeerWireMsg::ClusterPropose { .. } => "cluster_propose",
        PeerWireMsg::ClusterAttest { .. } => "cluster_attest",
        PeerWireMsg::SyncProfileAnnounce { .. } => "sync_profile_announce",
        PeerWireMsg::SyncTipAnnounce { .. } => "sync_tip_announce",
        PeerWireMsg::SyncHeadersReq { .. } => "sync_headers_req",
        PeerWireMsg::SyncHeadersBatch { .. } => "sync_headers_batch",
        PeerWireMsg::SyncBlocksReq { .. } => "sync_blocks_req",
        PeerWireMsg::SyncBlocksBatch { .. } => "sync_blocks_batch",
        PeerWireMsg::SyncTxAnnounce { .. } => "sync_tx_announce",
        PeerWireMsg::SyncTxReq { .. } => "sync_tx_req",
        PeerWireMsg::SyncTxBatch { .. } => "sync_tx_batch",
        PeerWireMsg::SyncNack { .. } => "sync_nack",
        PeerWireMsg::SyncCatchupReq { .. } => "sync_catchup_req",
        PeerWireMsg::SyncCatchupChunk { .. } => "sync_catchup_chunk",
        PeerWireMsg::SyncCatchupDone { .. } => "sync_catchup_done",
    }
}

fn elapsed_ms_2dp(start: &std::time::Instant) -> f64 {
    (start.elapsed().as_micros() as f64) / 1000.0
}

async fn peer_focus_verbose(app: &App) -> bool {
    let ovr = app.log_ovr.read().await;
    let Some(row) = ovr.as_ref() else {
        return false;
    };
    let is_focus = row.focus == "transport:peers" || row.focus == "all";
    let is_verbose = row.level == "debug" || row.level == "trace";
    is_focus && is_verbose
}

pub(crate) async fn process_inbound_socket(
    app: &App,
    cfg: &TransportConfig,
    mut stream: tokio::net::TcpStream,
    peer: std::net::SocketAddr,
) {
    const INBOUND_READ_POLL_MS: u64 = 50;
    const INBOUND_READ_DRAIN_MS: u64 = 50;
    const INBOUND_READ_SLOW_MS: f64 = 500.0;
    let hs_start_at = std::time::Instant::now();
    let hs_start_ms = current_time_ms().unwrap_or(0);
    let local_addr = stream.local_addr().ok();
    let peer_key = peer.to_string();
    info!(
        target: "pwmd::peer",
        "peer tcp connect succeeded seed=inbound local={:?} remote={}",
        local_addr, peer
    );
    info!(
        target: "pwmd::peer",
        "peer handshake started seed=inbound node_id=unknown domain_hi=unknown remote={} ts_ms={}",
        peer,
        hs_start_ms
    );
    info!(
        target: "pwmd::peer",
        "peer handshake step=wait_hello seed=inbound remote={} ts_ms={} expect=hello timeout_ms={}",
        peer,
        hs_start_ms,
        cfg.handshake_timeout_ms
    );
    let hello_wait_start_at = std::time::Instant::now();
    let hello = match read_wire_msg(&mut stream, cfg.handshake_timeout_ms).await {
        Ok(PeerWireMsg::Hello { node_hello }) => {
            let hello_recv_ms = current_time_ms().unwrap_or(hs_start_ms);
            info!(
                target: "pwmd::peer",
                "peer handshake step=recv_hello seed=inbound remote={} ts_ms={} latency_ms={:.2} hello_fields={}",
                peer,
                hello_recv_ms,
                elapsed_ms_2dp(&hello_wait_start_at),
                hello_trace_fields(&node_hello)
            );
            node_hello
        }
        Ok(_) => {
            let fail_ms = current_time_ms().unwrap_or(hs_start_ms);
            warn!(
                target: "pwmd::peer",
                "peer handshake failed seed=inbound node_id=unknown reason=expected_hello remote={} ts_ms={} latency_ms={:.2} timeout_ms={}",
                peer,
                fail_ms,
                elapsed_ms_2dp(&hello_wait_start_at),
                cfg.handshake_timeout_ms
            );
            info!(
                target: "pwmd::peer",
                "peer handshake step=send_hello_ack_reject seed=inbound remote={} ts_ms={} reason=expected_hello timeout_ms={}",
                peer,
                fail_ms,
                cfg.handshake_timeout_ms
            );
            let _ = write_wire_msg(
                &mut stream,
                &PeerWireMsg::HelloAck {
                    accepted: false,
                    reason: Some("expected_hello".to_string()),
                    node_hello: None,
                },
                cfg.handshake_timeout_ms,
            )
            .await;
            let mut hs = handshake_write_traced(app, "inbound").await;
            record_peer_close(
                &mut hs,
                current_time_ms().unwrap_or(0),
                "inbound",
                None,
                PeerCloseReason::ProtocolError,
                Some("expected_hello"),
            );
            return;
        }
        Err(err) => {
            let fail_ms = current_time_ms().unwrap_or(hs_start_ms);
            warn!(
                target: "pwmd::peer",
                "peer handshake failed seed=inbound node_id=unknown reason=hello_read_failed ts_ms={} latency_ms={:.2} timeout_ms={} detail={}",
                fail_ms,
                elapsed_ms_2dp(&hello_wait_start_at),
                cfg.handshake_timeout_ms,
                err
            );
            let mut hs = handshake_write_traced(app, "inbound").await;
            record_peer_close(
                &mut hs,
                current_time_ms().unwrap_or(0),
                "inbound",
                None,
                wire_close_reason(&err),
                Some(detail_with_err("hello_read_failed", &err).as_str()),
            );
            return;
        }
    };
    let now_ms = match current_time_ms() {
        Ok(v) => v,
        Err(_) => 0,
    };
    let validate_stage_start_at = std::time::Instant::now();
    let validate_stage_start_ms = current_time_ms().unwrap_or(now_ms);
    let expected_bridge = crate::bridge_trust::local_bridge_commitment(app).await;
    let expected_bridge_ready_ms = current_time_ms().unwrap_or(validate_stage_start_ms);
    info!(
        target: "pwmd::peer",
        "peer handshake step=validate_context_ready seed=inbound remote={} ts_ms={} latency_ms={:.2} node_id={} expected_bridge_len={}",
        peer,
        expected_bridge_ready_ms,
        elapsed_ms_2dp(&validate_stage_start_at),
        hello.node.node_id,
        expected_bridge.len()
    );
    let mut reject_reason: Option<String> = None;
    {
        info!(
            target: "pwmd::peer",
            "peer handshake step=validate_remote_hello seed=inbound remote={} ts_ms={} node_id={} domain_hi=0x{:02X}",
            peer,
            now_ms,
            hello.node.node_id,
            hello.cluster.domain_hi
        );
        let hs_lock_wait_start_at = std::time::Instant::now();
        let hs_lock_wait_start_ms = current_time_ms().unwrap_or(now_ms);
        let mut hs = handshake_write_traced(app, "inbound").await;
        let hs_lock_acquired_ms = current_time_ms().unwrap_or(hs_lock_wait_start_ms);
        info!(
            target: "pwmd::peer",
            "peer handshake step=validate_state_lock_acquired seed=inbound remote={} ts_ms={} latency_ms={:.2} node_id={}",
            peer,
            hs_lock_acquired_ms,
            elapsed_ms_2dp(&hs_lock_wait_start_at),
            hello.node.node_id
        );
        let validate_remote_start_at = std::time::Instant::now();
        let validate_remote_start_ms = current_time_ms().unwrap_or(hs_lock_acquired_ms);
        match process_incoming_peer_hello(
            &mut hs,
            &hello,
            now_ms,
            &peer.to_string(),
            false,
            Some(expected_bridge.as_str()),
            app.identity.cluster_id.as_str(),
        ) {
            Ok(_) => {
                hs.transport.snapshot.session_untrusted_total = hs
                    .transport
                    .snapshot
                    .session_untrusted_total
                    .saturating_add(1);
                let validate_ok_ms = current_time_ms().unwrap_or(validate_remote_start_ms);
                info!(
                    target: "pwmd::peer",
                    "peer handshake step=validate_remote_hello_ok seed=inbound remote={} ts_ms={} latency_ms={:.2} node_id={}",
                    peer,
                    validate_ok_ms,
                    elapsed_ms_2dp(&validate_remote_start_at),
                    hello.node.node_id
                );
            }
            Err(reason) => {
                let validate_reject_ms = current_time_ms().unwrap_or(validate_remote_start_ms);
                warn!(
                    target: "pwmd::peer",
                    "peer handshake step=validate_remote_hello_reject seed=inbound remote={} ts_ms={} latency_ms={:.2} node_id={} reason={}",
                    peer,
                    validate_reject_ms,
                    elapsed_ms_2dp(&validate_remote_start_at),
                    hello.node.node_id,
                    reason
                );
                reject_reason = Some(reason);
            }
        }
        let hs_lock_release_ms = current_time_ms().unwrap_or(hs_lock_acquired_ms);
        info!(
            target: "pwmd::peer",
            "peer handshake step=validate_state_lock_released seed=inbound remote={} ts_ms={} held_ms={} node_id={}",
            peer,
            hs_lock_release_ms,
            hs_lock_release_ms.saturating_sub(hs_lock_acquired_ms),
            hello.node.node_id
        );
    }
    let validate_stage_done_ms = current_time_ms().unwrap_or(validate_stage_start_ms);
    info!(
        target: "pwmd::peer",
        "peer handshake step=validate_stage_done seed=inbound remote={} ts_ms={} latency_ms={:.2} node_id={} reject={}",
        peer,
        validate_stage_done_ms,
        elapsed_ms_2dp(&validate_stage_start_at),
        hello.node.node_id,
        reject_reason.is_some()
    );
    if let Some(reason) = reject_reason {
        info!(
            target: "pwmd::peer",
            "peer handshake step=send_hello_ack_reject seed=inbound node_id={} remote={} ts_ms={} reason={} timeout_ms={}",
            hello.node.node_id,
            peer,
            now_ms,
            reason,
            cfg.handshake_timeout_ms
        );
        warn!(
            target: "pwmd::peer",
            "peer handshake rejected seed=inbound node_id={} domain_hi=0x{:02X} ts_ms={} reason={}",
            hello.node.node_id,
            hello.cluster.domain_hi,
            now_ms,
            reason
        );
        let _ = write_wire_msg(
            &mut stream,
            &PeerWireMsg::HelloAck {
                accepted: false,
                reason: Some(reason.clone()),
                node_hello: None,
            },
            cfg.handshake_timeout_ms,
        )
        .await;
        let mut hs = handshake_write_traced(app, "inbound").await;
        record_peer_close(
            &mut hs,
            now_ms,
            "inbound",
            Some(&hello.node.node_id),
            PeerCloseReason::HandshakeRejected,
            Some(reason.as_str()),
        );
        return;
    }
    let local_hello_prep_start_at = std::time::Instant::now();
    let genesis_hash = {
        let hs = crate::transport::handshake_read_traced(app, "inbound").await;
        hs.validation_ctx.expected_genesis_hash.clone()
    };
    let chain_tip_height = {
        let g = app.inner.read().await;
        Some(g.chain.tip_h())
    };
    let bridge_commitment = crate::bridge_trust::local_bridge_commitment(app).await;
    let local_hello_ready_ms = current_time_ms().unwrap_or(now_ms);
    info!(
        target: "pwmd::peer",
        "peer handshake step=prepare_local_hello_done seed=inbound node_id={} remote={} ts_ms={} latency_ms={:.2} chain_tip_h={:?}",
        hello.node.node_id,
        peer,
        local_hello_ready_ms,
        elapsed_ms_2dp(&local_hello_prep_start_at),
        chain_tip_height
    );
    let local_hello = build_local_node_hello(
        app,
        genesis_hash,
        Some(bridge_commitment),
        now_ms,
        chain_tip_height,
    );
    info!(
        target: "pwmd::peer",
        "peer handshake step=send_hello_ack_accept seed=inbound node_id={} remote={} ts_ms={} timeout_ms={} hello_fields={}",
        hello.node.node_id,
        peer,
        now_ms,
        cfg.handshake_timeout_ms,
        hello_trace_fields(&local_hello)
    );
    let ack_send_start_at = std::time::Instant::now();
    if let Err(err) = write_wire_msg(
        &mut stream,
        &PeerWireMsg::HelloAck {
            accepted: true,
            reason: None,
            node_hello: Some(local_hello),
        },
        cfg.handshake_timeout_ms,
    )
    .await
    {
        let fail_ms = current_time_ms().unwrap_or(now_ms);
        warn!(
            target: "pwmd::peer",
            "peer handshake failed seed=inbound node_id={} reason=hello_ack_write_failed ts_ms={} latency_ms={:.2} timeout_ms={} detail={}",
            hello.node.node_id,
            fail_ms,
            elapsed_ms_2dp(&ack_send_start_at),
            cfg.handshake_timeout_ms,
            err
        );
        let mut hs = handshake_write_traced(app, "inbound").await;
        record_peer_close(
            &mut hs,
            now_ms,
            "inbound",
            Some(&hello.node.node_id),
            PeerCloseReason::HandshakeFailure,
            Some(detail_with_err("hello_ack_write_failed", &err).as_str()),
        );
        return;
    }
    let ack_sent_ms = current_time_ms().unwrap_or(now_ms);
    info!(
        target: "pwmd::peer",
        "peer handshake step=hello_ack_sent seed=inbound node_id={} remote={} ts_ms={} latency_ms={:.2} timeout_ms={}",
        hello.node.node_id,
        peer,
        ack_sent_ms,
        elapsed_ms_2dp(&ack_send_start_at),
        cfg.handshake_timeout_ms
    );
    info!(
        target: "pwmd::peer",
        "peer handshake completed seed=inbound node_id={} domain_hi=0x{:02X} ts_ms={} latency_ms={:.2}",
        hello.node.node_id,
        hello.cluster.domain_hi,
        ack_sent_ms,
        elapsed_ms_2dp(&hs_start_at)
    );
    info!(
        target: "pwmd::peer",
        "peer session open seed=inbound node_id={} domain_hi=0x{:02X}",
        hello.node.node_id, hello.cluster.domain_hi
    );
    let sync_v1 = peer_sync_v1(&hello);
    let same_shard = hello.cluster.domain_hi == app.identity.cluster_domain_hi;
    let (sync_hdr_cap, sync_blk_cap) = sync_live::sync_caps(&hello);
    let can_cup = hello
        .capabilities
        .sync_profile
        .as_ref()
        .map(|x| x.supports_epoch_catchup)
        .unwrap_or(false);
    let mut sync_seq_no = 0u64;
    info!(
        target: "pwmd::peer",
        "peer sync mode negotiated seed=inbound node_id={} mode={}",
        hello.node.node_id,
        sync_mode_text(&hello)
    );
    let mut wait_logged = false;
    // Guard against initial sync racing with snapshot restore.
    while !app.init.read().await.is_ready() {
        if !wait_logged {
            info!(
                target: "pwmd::peer",
                "peer session waiting for init ready seed=inbound node_id={}",
                hello.node.node_id
            );
            wait_logged = true;
        }
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }
    if let Err(err) = send_cross_shard_facts(app, cfg, &mut stream).await {
        let mut hs = handshake_write_traced(app, "inbound").await;
        record_peer_close(
            &mut hs,
            current_time_ms().unwrap_or(now_ms),
            &peer_key,
            Some(&hello.node.node_id),
            wire_close_reason(&err),
            Some(detail_with_err("cross_shard_facts_write_failed", &err).as_str()),
        );
        return;
    }
    if let Err(err) = send_account_views(app, cfg, &mut stream).await {
        let mut hs = handshake_write_traced(app, "inbound").await;
        record_peer_close(
            &mut hs,
            current_time_ms().unwrap_or(now_ms),
            &peer_key,
            Some(&hello.node.node_id),
            wire_close_reason(&err),
            Some(detail_with_err("account_views_write_failed", &err).as_str()),
        );
        return;
    }
    if let Err(err) = send_cluster_prop(app, cfg, &mut stream, &hello).await {
        let mut hs = handshake_write_traced(app, "inbound").await;
        record_peer_close(
            &mut hs,
            current_time_ms().unwrap_or(now_ms),
            &peer_key,
            Some(&hello.node.node_id),
            wire_close_reason(&err),
            Some(detail_with_err("cluster_propose_write_failed", &err).as_str()),
        );
        return;
    }
    let mut probe_ticks = 0u64;
    let mut probe_rx_total = 0u64;
    let mut probe_rx_timeout_total = 0u64;
    let mut probe_rx_err_total = 0u64;
    let mut probe_hb = 0u64;
    let mut probe_hb_ack = 0u64;
    let mut probe_cluster = 0u64;
    let mut probe_sync = 0u64;
    let mut probe_other = 0u64;
    let mut probe_window_rx = 0u64;
    let mut probe_window_timeout = 0u64;
    let mut probe_window_err = 0u64;
    let mut probe_window_hb = 0u64;
    let mut probe_window_hb_ack = 0u64;
    let mut probe_window_cluster = 0u64;
    let mut probe_window_sync = 0u64;
    let mut probe_window_other = 0u64;
    let mut probe_last_rx_tick = 0u64;
    let (close_reason, close_detail) = loop {
        probe_ticks = probe_ticks.saturating_add(1);
        let read_probe = tokio::time::timeout(
            std::time::Duration::from_millis(INBOUND_READ_POLL_MS),
            stream.readable(),
        )
        .await;
        match read_probe {
            Err(_) => {
                probe_rx_timeout_total = probe_rx_timeout_total.saturating_add(1);
                probe_window_timeout = probe_window_timeout.saturating_add(1);
                if probe_ticks % 100 == 0 {
                    info!(
                        target: "pwmd::peer",
                        "inbound_fast_loop_probe node_id={} remote={} ticks={} no_rx_ticks={} window_rx={} window_timeout={} window_err={} window_hb={} window_hb_ack={} window_cluster={} window_sync={} window_other={} total_rx={} total_timeout={} total_err={} total_hb={} total_hb_ack={} total_cluster={} total_sync={} total_other={}",
                        hello.node.node_id,
                        peer,
                        probe_ticks,
                        probe_ticks.saturating_sub(probe_last_rx_tick),
                        probe_window_rx,
                        probe_window_timeout,
                        probe_window_err,
                        probe_window_hb,
                        probe_window_hb_ack,
                        probe_window_cluster,
                        probe_window_sync,
                        probe_window_other,
                        probe_rx_total,
                        probe_rx_timeout_total,
                        probe_rx_err_total,
                        probe_hb,
                        probe_hb_ack,
                        probe_cluster,
                        probe_sync,
                        probe_other,
                    );
                    probe_window_rx = 0;
                    probe_window_timeout = 0;
                    probe_window_err = 0;
                    probe_window_hb = 0;
                    probe_window_hb_ack = 0;
                    probe_window_cluster = 0;
                    probe_window_sync = 0;
                    probe_window_other = 0;
                }
                continue;
            }
            Ok(Err(err)) => {
                let err = format!("wire_readable_failed: {err}");
                probe_rx_err_total = probe_rx_err_total.saturating_add(1);
                probe_window_err = probe_window_err.saturating_add(1);
                warn!(
                    target: "pwmd::peer",
                    "inbound_fast_loop_probe_close node_id={} remote={} ticks={} no_rx_ticks={} window_rx={} window_timeout={} window_err={} total_rx={} total_timeout={} total_err={}",
                    hello.node.node_id,
                    peer,
                    probe_ticks,
                    probe_ticks.saturating_sub(probe_last_rx_tick),
                    probe_window_rx,
                    probe_window_timeout,
                    probe_window_err,
                    probe_rx_total,
                    probe_rx_timeout_total,
                    probe_rx_err_total,
                );
                break (
                    wire_close_reason(&err),
                    detail_with_err("wire_readable_failed", &err),
                );
            }
            Ok(Ok(())) => {}
        }
        let wire_read_start_at = std::time::Instant::now();
        let wire_read_start_ms = current_time_ms().unwrap_or(now_ms);
        match read_wire_msg(&mut stream, INBOUND_READ_DRAIN_MS).await {
            Ok(msg) => {
                let wire_read_done_ms = current_time_ms().unwrap_or(wire_read_start_ms);
                let wire_read_latency_ms = elapsed_ms_2dp(&wire_read_start_at);
                let should_log_read = if wire_read_latency_ms > INBOUND_READ_SLOW_MS {
                    true
                } else {
                    peer_focus_verbose(app).await
                };
                if should_log_read {
                    info!(
                        target: "pwmd::peer",
                        "peer inbound socket_read node_id={} remote={} tick={} ts_ms={} latency_ms={:.2} timeout_ms={} msg={}",
                        hello.node.node_id,
                        peer,
                        probe_ticks,
                        wire_read_done_ms,
                        wire_read_latency_ms,
                        INBOUND_READ_DRAIN_MS,
                        peer_wire_msg_kind(&msg),
                    );
                }
                match msg {
                    PeerWireMsg::Heartbeat {
                        unix_ms,
                        chain_tip_height,
                        federation_shard_id,
                        federation_gossip,
                        ..
                    } => {
                        probe_rx_total = probe_rx_total.saturating_add(1);
                        probe_window_rx = probe_window_rx.saturating_add(1);
                        probe_hb = probe_hb.saturating_add(1);
                        probe_window_hb = probe_window_hb.saturating_add(1);
                        probe_last_rx_tick = probe_ticks;
                        crate::federation::merge_remote_hb(
                            app,
                            false,
                            &hello,
                            &hello.node.node_id,
                            unix_ms,
                            chain_tip_height,
                            federation_shard_id,
                            federation_gossip,
                        )
                        .await;
                        if let Err(err) = write_wire_msg(
                            &mut stream,
                            &PeerWireMsg::HeartbeatAck { unix_ms },
                            cfg.heartbeat_timeout_ms,
                        )
                        .await
                        {
                            break (
                                wire_close_reason(&err),
                                detail_with_err("heartbeat_ack_write_failed", &err),
                            );
                        }
                        if let Err(err) = send_cross_shard_facts(app, cfg, &mut stream).await {
                            break (
                                wire_close_reason(&err),
                                detail_with_err("cross_shard_facts_write_failed", &err),
                            );
                        }
                        if let Err(err) = send_account_views(app, cfg, &mut stream).await {
                            break (
                                wire_close_reason(&err),
                                detail_with_err("account_views_write_failed", &err),
                            );
                        }
                        if let Err(err) =
                            send_sync_tx_batch(app, cfg, &mut stream, &hello, &mut sync_seq_no)
                                .await
                        {
                            break (
                                wire_close_reason(&err),
                                detail_with_err("sync_tx_batch_write_failed", &err),
                            );
                        }
                        if let Err(err) = send_cluster_prop(app, cfg, &mut stream, &hello).await {
                            break (
                                wire_close_reason(&err),
                                detail_with_err("cluster_propose_write_failed", &err),
                            );
                        }
                        if let Err(err) = sync_live::send_sync_tip(
                            app,
                            cfg,
                            &mut stream,
                            &hello,
                            &mut sync_seq_no,
                        )
                        .await
                        {
                            break (
                                wire_close_reason(&err),
                                detail_with_err("sync_tip_write_failed", &err),
                            );
                        }
                    }
                    PeerWireMsg::HeartbeatAck { .. } => {
                        probe_rx_total = probe_rx_total.saturating_add(1);
                        probe_window_rx = probe_window_rx.saturating_add(1);
                        probe_hb_ack = probe_hb_ack.saturating_add(1);
                        probe_window_hb_ack = probe_window_hb_ack.saturating_add(1);
                        probe_last_rx_tick = probe_ticks;
                    }
                    PeerWireMsg::CrossShardFacts { facts } => {
                        probe_rx_total = probe_rx_total.saturating_add(1);
                        probe_window_rx = probe_window_rx.saturating_add(1);
                        probe_other = probe_other.saturating_add(1);
                        probe_window_other = probe_window_other.saturating_add(1);
                        probe_last_rx_tick = probe_ticks;
                        merge_cross_shard_facts(app, facts, false).await;
                    }
                    PeerWireMsg::AccountViews { .. } => {
                        probe_rx_total = probe_rx_total.saturating_add(1);
                        probe_window_rx = probe_window_rx.saturating_add(1);
                        probe_other = probe_other.saturating_add(1);
                        probe_window_other = probe_window_other.saturating_add(1);
                        probe_last_rx_tick = probe_ticks;
                    }
                    msg @ (PeerWireMsg::ClusterPropose { .. }
                    | PeerWireMsg::ClusterAttest { .. }) => {
                        probe_rx_total = probe_rx_total.saturating_add(1);
                        probe_window_rx = probe_window_rx.saturating_add(1);
                        probe_cluster = probe_cluster.saturating_add(1);
                        probe_window_cluster = probe_window_cluster.saturating_add(1);
                        probe_last_rx_tick = probe_ticks;
                        let route_cluster_start_at = std::time::Instant::now();
                        let route_cluster_start_ms = current_time_ms().unwrap_or(now_ms);
                        let maybe_attest = route_cluster_stub(app, &hello.node.node_id, msg).await;
                        let route_cluster_done_ms =
                            current_time_ms().unwrap_or(route_cluster_start_ms);
                        let route_cluster_latency_ms = elapsed_ms_2dp(&route_cluster_start_at);
                        if route_cluster_latency_ms >= 100.0 {
                            warn!(
                                target: "pwmd::peer",
                                "peer inbound cluster_route_slow node_id={} remote={} tick={} ts_ms={} latency_ms={:.2}",
                                hello.node.node_id,
                                peer,
                                probe_ticks,
                                route_cluster_done_ms,
                                route_cluster_latency_ms,
                            );
                        }
                        if let Some(attest) = maybe_attest {
                            let h = attest.height;
                            let r = attest.round;
                            if let Err(err) = write_wire_msg(
                                &mut stream,
                                &PeerWireMsg::ClusterAttest { msg: attest },
                                cfg.heartbeat_timeout_ms,
                            )
                            .await
                            {
                                break (
                                    wire_close_reason(&err),
                                    detail_with_err("cluster_attest_write_failed", &err),
                                );
                            }
                            if let Some(bt) = app.block_timing.as_ref() {
                                crate::block_timing::note_att_wire(
                                    bt,
                                    crate::block_timing::AttCtx {
                                        h,
                                        r,
                                        t_ms: crate::block_timing::now_ms_f64(),
                                        att_id: app.node_instance_id.clone(),
                                    },
                                );
                            }
                        }
                    }
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
                    | PeerWireMsg::SyncCatchupDone { .. }) => {
                        probe_rx_total = probe_rx_total.saturating_add(1);
                        probe_window_rx = probe_window_rx.saturating_add(1);
                        probe_sync = probe_sync.saturating_add(1);
                        probe_window_sync = probe_window_sync.saturating_add(1);
                        probe_last_rx_tick = probe_ticks;
                        let outcome = route_sync_stub(
                            app,
                            cfg,
                            &mut stream,
                            None,
                            &hello.node.node_id,
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
                        if let super::SyncRouteOutcome::Disconnect { reason, detail } = outcome {
                            break (reason, detail);
                        }
                    }
                    PeerWireMsg::Hello { .. } | PeerWireMsg::HelloAck { .. } => {
                        break (
                            PeerCloseReason::ProtocolError,
                            "unexpected_handshake_frame".to_string(),
                        );
                    }
                }
            }
            Err(err) => {
                let wire_read_fail_ms = current_time_ms().unwrap_or(wire_read_start_ms);
                let wire_read_latency_ms = elapsed_ms_2dp(&wire_read_start_at);
                if is_wire_timeout(&err) {
                    info!(
                        target: "pwmd::peer",
                        "peer inbound socket_read_timeout node_id={} remote={} tick={} ts_ms={} latency_ms={:.2} timeout_ms={} detail={}",
                        hello.node.node_id,
                        peer,
                        probe_ticks,
                        wire_read_fail_ms,
                        wire_read_latency_ms,
                        INBOUND_READ_DRAIN_MS,
                        err,
                    );
                    probe_rx_timeout_total = probe_rx_timeout_total.saturating_add(1);
                    probe_window_timeout = probe_window_timeout.saturating_add(1);
                    if probe_ticks % 100 == 0 {
                        info!(
                            target: "pwmd::peer",
                            "inbound_fast_loop_probe node_id={} remote={} ticks={} no_rx_ticks={} window_rx={} window_timeout={} window_err={} window_hb={} window_hb_ack={} window_cluster={} window_sync={} window_other={} total_rx={} total_timeout={} total_err={} total_hb={} total_hb_ack={} total_cluster={} total_sync={} total_other={}",
                            hello.node.node_id,
                            peer,
                            probe_ticks,
                            probe_ticks.saturating_sub(probe_last_rx_tick),
                            probe_window_rx,
                            probe_window_timeout,
                            probe_window_err,
                            probe_window_hb,
                            probe_window_hb_ack,
                            probe_window_cluster,
                            probe_window_sync,
                            probe_window_other,
                            probe_rx_total,
                            probe_rx_timeout_total,
                            probe_rx_err_total,
                            probe_hb,
                            probe_hb_ack,
                            probe_cluster,
                            probe_sync,
                            probe_other,
                        );
                        probe_window_rx = 0;
                        probe_window_timeout = 0;
                        probe_window_err = 0;
                        probe_window_hb = 0;
                        probe_window_hb_ack = 0;
                        probe_window_cluster = 0;
                        probe_window_sync = 0;
                        probe_window_other = 0;
                    }
                    continue;
                }
                warn!(
                    target: "pwmd::peer",
                    "peer inbound socket_read_failed node_id={} remote={} tick={} ts_ms={} latency_ms={:.2} timeout_ms={} detail={}",
                    hello.node.node_id,
                    peer,
                    probe_ticks,
                    wire_read_fail_ms,
                    wire_read_latency_ms,
                    INBOUND_READ_DRAIN_MS,
                    err,
                );
                probe_rx_err_total = probe_rx_err_total.saturating_add(1);
                probe_window_err = probe_window_err.saturating_add(1);
                warn!(
                    target: "pwmd::peer",
                    "inbound_fast_loop_probe_close node_id={} remote={} ticks={} no_rx_ticks={} window_rx={} window_timeout={} window_err={} total_rx={} total_timeout={} total_err={}",
                    hello.node.node_id,
                    peer,
                    probe_ticks,
                    probe_ticks.saturating_sub(probe_last_rx_tick),
                    probe_window_rx,
                    probe_window_timeout,
                    probe_window_err,
                    probe_rx_total,
                    probe_rx_timeout_total,
                    probe_rx_err_total,
                );
                if let Some(reason) = sync_wire_reason(&err) {
                    let mut hs = handshake_write_traced(app, "inbound").await;
                    hs.transport.snapshot.sync_v1_drop_total =
                        hs.transport.snapshot.sync_v1_drop_total.saturating_add(1);
                    increment_string_u64_bucket(
                        &mut hs.transport.snapshot.sync_v1_drop_reason,
                        reason,
                    );
                    warn!(
                        target: "pwmd::peer",
                        "peer sync frame dropped node_id={} reason={} detail={}",
                        hello.node.node_id,
                        reason,
                        err
                    );
                }
                break (
                    wire_close_reason(&err),
                    detail_with_err("wire_read_failed", &err),
                );
            }
        }
    };
    let mut hs = handshake_write_traced(app, "inbound").await;
    record_peer_close(
        &mut hs,
        current_time_ms().unwrap_or(now_ms),
        &peer_key,
        Some(&hello.node.node_id),
        close_reason,
        Some(close_detail.as_str()),
    );
}
