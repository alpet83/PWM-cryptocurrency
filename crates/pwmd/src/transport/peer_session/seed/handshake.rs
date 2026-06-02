//! Completes outbound peer hello/ack exchange after TCP connects.

use super::super::super::*;
use super::super::{
    handshake_write_traced, mark_trusted_peer_live, read_wire_msg, write_wire_msg, PeerWireMsg,
};
use crate::handshake::NodeHello;

fn hello_sync_profile_text(hello: &NodeHello) -> String {
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

fn hello_trace_fields(hello: &NodeHello) -> String {
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

pub(super) async fn seed_finish_handshake(
    app: &App,
    cfg: &TransportConfig,
    seed: std::net::SocketAddr,
    seed_key: &str,
    now_ms: u64,
    stream: &mut tokio::net::TcpStream,
) -> Option<NodeHello> {
    let hs_start_ms = current_time_ms().unwrap_or(now_ms);
    let genesis_hash = {
        let hs = crate::transport::handshake_read_traced(app, "seed_handshake").await;
        hs.validation_ctx.expected_genesis_hash.clone()
    };
    let chain_tip_height = {
        let g = app.inner.read().await;
        Some(g.chain.tip_h())
    };
    let bridge_commitment = crate::bridge_trust::local_bridge_commitment(app).await;
    let local_hello = build_local_node_hello(
        &app,
        genesis_hash,
        Some(bridge_commitment),
        now_ms,
        chain_tip_height,
    );
    let expected_bridge_commitment = local_hello.bridge_commitment.clone();
    let hello_trace = hello_trace_fields(&local_hello);
    info!(
        target: "pwmd::peer",
        "peer handshake started seed={} node_id={} domain_hi=0x{:02X} ts_ms={}",
        seed_key,
        app.identity.node_id,
        app.identity.cluster_domain_hi,
        hs_start_ms
    );
    info!(
        target: "pwmd::peer",
        "peer handshake step=send_hello seed={} ts_ms={} hello_fields={}",
        seed_key,
        hs_start_ms,
        hello_trace
    );
    let hello_send_start_ms = current_time_ms().unwrap_or(hs_start_ms);
    if let Err(err) = write_wire_msg(
        stream,
        &PeerWireMsg::Hello {
            node_hello: local_hello,
        },
        cfg.handshake_timeout_ms,
    )
    .await
    {
        let fail_ms = current_time_ms().unwrap_or(now_ms);
        let mut hs = handshake_write_traced(app, "seed_handshake").await;
        hs.transport.snapshot.session_retrying_total = hs
            .transport
            .snapshot
            .session_retrying_total
            .saturating_add(1);
        warn!(
            target: "pwmd::peer",
            "peer handshake failed seed={} node_id={} reason=hello_write_failed ts_ms={} latency_ms={} timeout_ms={} detail={}",
            seed_key,
            app.identity.node_id,
            fail_ms,
            fail_ms.saturating_sub(hello_send_start_ms),
            cfg.handshake_timeout_ms,
            err
        );
        set_peer_error(
            &mut hs,
            now_ms,
            format!("seed {seed} wire_hello_write_failed: {err}"),
        );
        record_peer_close(
            &mut hs,
            now_ms,
            seed_key,
            None,
            wire_close_reason(&err),
            Some(detail_with_err("hello_write_failed", &err).as_str()),
        );
        record_reconnect(
            &mut hs,
            now_ms,
            seed_key,
            PeerReconnectReason::HandshakeFailure,
            Some(detail_with_err("hello_write_failed", &err).as_str()),
        );
        drop(hs);
        tokio::time::sleep(std::time::Duration::from_millis(
            super::super::peer_retry_sleep_ms(cfg, seed_key, now_ms),
        ))
        .await;
        return None;
    }
    let hello_sent_ms = current_time_ms().unwrap_or(now_ms);
    info!(
        target: "pwmd::peer",
        "peer handshake step=hello_sent seed={} ts_ms={} latency_ms={} timeout_ms={}",
        seed_key,
        hello_sent_ms,
        hello_sent_ms.saturating_sub(hello_send_start_ms),
        cfg.handshake_timeout_ms
    );
    info!(
        target: "pwmd::peer",
        "peer handshake step=wait_hello_ack seed={} ts_ms={} expect=hello_ack timeout_ms={}",
        seed_key,
        hello_sent_ms,
        cfg.handshake_timeout_ms
    );
    let ack_wait_start_ms = current_time_ms().unwrap_or(hello_sent_ms);
    let ack = read_wire_msg(stream, cfg.handshake_timeout_ms).await;
    let remote = match ack {
        Ok(PeerWireMsg::HelloAck {
            accepted: true,
            node_hello: Some(v),
            ..
        }) => {
            let ack_recv_ms = current_time_ms().unwrap_or(now_ms);
            info!(
                target: "pwmd::peer",
                "peer handshake step=recv_hello_ack seed={} ts_ms={} latency_ms={} hello_fields={}",
                seed_key,
                ack_recv_ms,
                ack_recv_ms.saturating_sub(ack_wait_start_ms),
                hello_trace_fields(&v)
            );
            v
        }
        Ok(PeerWireMsg::HelloAck { reason, .. }) => {
            let ack_recv_ms = current_time_ms().unwrap_or(now_ms);
            let reason = reason.unwrap_or_else(|| "unknown".to_string());
            let mut hs = handshake_write_traced(app, "seed_handshake").await;
            hs.transport.snapshot.session_retrying_total = hs
                .transport
                .snapshot
                .session_retrying_total
                .saturating_add(1);
            set_peer_error(
                &mut hs,
                now_ms,
                format!("seed {seed} hello_rejected reason={reason}"),
            );
            warn!(
                target: "pwmd::peer",
                "peer handshake rejected seed={} node_id=unknown domain_hi=unknown ts_ms={} latency_ms={} reason={}",
                seed_key,
                ack_recv_ms,
                ack_recv_ms.saturating_sub(ack_wait_start_ms),
                reason
            );
            record_peer_close(
                &mut hs,
                now_ms,
                seed_key,
                None,
                PeerCloseReason::HandshakeRejected,
                Some(reason.as_str()),
            );
            record_reconnect(
                &mut hs,
                now_ms,
                seed_key,
                PeerReconnectReason::HandshakeFailure,
                Some("hello_rejected"),
            );
            drop(hs);
            tokio::time::sleep(std::time::Duration::from_millis(
                super::super::peer_retry_sleep_ms(cfg, seed_key, now_ms),
            ))
            .await;
            return None;
        }
        Ok(other) => {
            let ack_recv_ms = current_time_ms().unwrap_or(now_ms);
            let mut hs = handshake_write_traced(app, "seed_handshake").await;
            hs.transport.snapshot.session_retrying_total = hs
                .transport
                .snapshot
                .session_retrying_total
                .saturating_add(1);
            set_peer_error(
                &mut hs,
                now_ms,
                format!("seed {seed} wire_hello_ack_unexpected: {other:?}"),
            );
            warn!(
                target: "pwmd::peer",
                "peer handshake failed seed={} node_id=unknown reason=unexpected_ack ts_ms={} latency_ms={} frame={:?}",
                seed_key,
                ack_recv_ms,
                ack_recv_ms.saturating_sub(ack_wait_start_ms),
                other
            );
            record_peer_close(
                &mut hs,
                now_ms,
                seed_key,
                None,
                PeerCloseReason::ProtocolError,
                Some("unexpected_ack"),
            );
            record_reconnect(
                &mut hs,
                now_ms,
                seed_key,
                PeerReconnectReason::ProtocolError,
                Some("unexpected_ack"),
            );
            drop(hs);
            tokio::time::sleep(std::time::Duration::from_millis(
                super::super::peer_retry_sleep_ms(cfg, seed_key, now_ms),
            ))
            .await;
            return None;
        }
        Err(err) => {
            let ack_fail_ms = current_time_ms().unwrap_or(now_ms);
            let mut hs = handshake_write_traced(app, "seed_handshake").await;
            hs.transport.snapshot.session_retrying_total = hs
                .transport
                .snapshot
                .session_retrying_total
                .saturating_add(1);
            set_peer_error(
                &mut hs,
                now_ms,
                format!("seed {seed} wire_hello_read_failed: {err}"),
            );
            let close_reason = wire_close_reason(&err);
            warn!(
                target: "pwmd::peer",
                "peer handshake failed seed={} node_id=unknown reason=hello_read_failed ts_ms={} latency_ms={} timeout_ms={} detail={}",
                seed_key,
                ack_fail_ms,
                ack_fail_ms.saturating_sub(ack_wait_start_ms),
                cfg.handshake_timeout_ms,
                err
            );
            record_peer_close(
                &mut hs,
                now_ms,
                seed_key,
                None,
                close_reason,
                Some(detail_with_err("hello_read_failed", &err).as_str()),
            );
            record_reconnect(
                &mut hs,
                now_ms,
                seed_key,
                reconnect_from_close(close_reason),
                Some(detail_with_err("hello_read_failed", &err).as_str()),
            );
            drop(hs);
            tokio::time::sleep(std::time::Duration::from_millis(
                super::super::peer_retry_sleep_ms(cfg, seed_key, now_ms),
            ))
            .await;
            return None;
        }
    };
    {
        let validate_start_ms = current_time_ms().unwrap_or(now_ms);
        info!(
            target: "pwmd::peer",
            "peer handshake step=validate_remote_hello seed={} ts_ms={} remote_node_id={} remote_domain_hi=0x{:02X}",
            seed_key,
            validate_start_ms,
            remote.node.node_id,
            remote.cluster.domain_hi
        );
        let mut hs = handshake_write_traced(app, "seed_handshake").await;
        if let Err(reason) = process_incoming_peer_hello(
            &mut hs,
            &remote,
            now_ms,
            &seed.to_string(),
            true,
            expected_bridge_commitment.as_deref(),
            app.identity.cluster_id.as_str(),
        ) {
            hs.transport.snapshot.session_retrying_total = hs
                .transport
                .snapshot
                .session_retrying_total
                .saturating_add(1);
            set_peer_error(
                &mut hs,
                now_ms,
                format!("seed {seed} remote_hello_rejected reason={reason}"),
            );
            warn!(
                target: "pwmd::peer",
                "peer handshake rejected seed={} node_id={} domain_hi=0x{:02X} ts_ms={} reason={}",
                seed_key,
                remote.node.node_id,
                remote.cluster.domain_hi,
                current_time_ms().unwrap_or(now_ms),
                reason
            );
            record_peer_close(
                &mut hs,
                now_ms,
                seed_key,
                Some(&remote.node.node_id),
                PeerCloseReason::HandshakeRejected,
                Some(reason.as_str()),
            );
            record_reconnect(
                &mut hs,
                now_ms,
                seed_key,
                PeerReconnectReason::HandshakeFailure,
                Some("remote_hello_rejected"),
            );
            drop(hs);
            tokio::time::sleep(std::time::Duration::from_millis(
                super::super::peer_retry_sleep_ms(cfg, seed_key, now_ms),
            ))
            .await;
            return None;
        }
        hs.transport.snapshot.session_connected_total = hs
            .transport
            .snapshot
            .session_connected_total
            .saturating_add(1);
        hs.transport.snapshot.session_trusted_total = hs
            .transport
            .snapshot
            .session_trusted_total
            .saturating_add(1);
        clear_peer_error(&mut hs);
        mark_seed_peer_node(&mut hs, seed_key, &remote.node.node_id);
        set_seed_due(&mut hs, seed_key, now_ms.saturating_add(cfg.retry_base_ms));
        mark_trusted_peer_live(&mut hs, &remote.node.node_id, now_ms);
        let hs_done_ms = current_time_ms().unwrap_or(now_ms);
        info!(
            target: "pwmd::peer",
            "peer handshake completed seed={} node_id={} domain_hi=0x{:02X} ts_ms={} latency_ms={}",
            seed_key,
            remote.node.node_id,
            remote.cluster.domain_hi,
            hs_done_ms,
            hs_done_ms.saturating_sub(hs_start_ms)
        );
        info!(
            target: "pwmd::peer",
            "peer session open seed={} node_id={} domain_hi=0x{:02X} ts_ms={}",
            seed_key,
            remote.node.node_id,
            remote.cluster.domain_hi,
            hs_done_ms
        );
    }
    crate::federation::merge_remote_hello(&app, &remote, now_ms).await;
    Some(remote)
}
