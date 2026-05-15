//! Completes outbound peer hello/ack exchange after TCP connects.

use super::super::super::*;
use super::super::{mark_trusted_peer_live, read_wire_msg, write_wire_msg, PeerWireMsg};
use crate::handshake::NodeHello;

pub(super) async fn seed_finish_handshake(
    app: &App,
    cfg: &TransportConfig,
    seed: std::net::SocketAddr,
    seed_key: &str,
    now_ms: u64,
    stream: &mut tokio::net::TcpStream,
) -> Option<NodeHello> {
    let genesis_hash = {
        let hs = app.handshake.read().await;
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
    info!(
        target: "pwmd::peer",
        "peer handshake started seed={} node_id={} domain_hi=0x{:02X}",
        seed_key, app.identity.node_id, app.identity.cluster_domain_hi
    );
    if let Err(err) = write_wire_msg(
        stream,
        &PeerWireMsg::Hello {
            node_hello: local_hello,
        },
        cfg.handshake_timeout_ms,
    )
    .await
    {
        let mut hs = app.handshake.write().await;
        hs.transport.snapshot.session_retrying_total = hs
            .transport
            .snapshot
            .session_retrying_total
            .saturating_add(1);
        warn!(
            target: "pwmd::peer",
            "peer handshake failed seed={} node_id={} reason=hello_write_failed detail={}",
            seed_key, app.identity.node_id, err
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
        tokio::time::sleep(std::time::Duration::from_millis(
            super::super::peer_retry_sleep_ms(cfg, seed_key, now_ms),
        ))
        .await;
        return None;
    }
    let ack = read_wire_msg(stream, cfg.handshake_timeout_ms).await;
    let remote = match ack {
        Ok(PeerWireMsg::HelloAck {
            accepted: true,
            node_hello: Some(v),
            ..
        }) => v,
        Ok(PeerWireMsg::HelloAck { reason, .. }) => {
            let reason = reason.unwrap_or_else(|| "unknown".to_string());
            let mut hs = app.handshake.write().await;
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
                "peer handshake rejected seed={} node_id=unknown domain_hi=unknown reason={}",
                seed_key, reason
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
            tokio::time::sleep(std::time::Duration::from_millis(
                super::super::peer_retry_sleep_ms(cfg, seed_key, now_ms),
            ))
            .await;
            return None;
        }
        Ok(other) => {
            let mut hs = app.handshake.write().await;
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
                "peer handshake failed seed={} node_id=unknown reason=unexpected_ack",
                seed_key
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
            tokio::time::sleep(std::time::Duration::from_millis(
                super::super::peer_retry_sleep_ms(cfg, seed_key, now_ms),
            ))
            .await;
            return None;
        }
        Err(err) => {
            let mut hs = app.handshake.write().await;
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
                "peer handshake failed seed={} node_id=unknown reason=hello_read_failed detail={}",
                seed_key, err
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
            tokio::time::sleep(std::time::Duration::from_millis(
                super::super::peer_retry_sleep_ms(cfg, seed_key, now_ms),
            ))
            .await;
            return None;
        }
    };
    {
        let mut hs = app.handshake.write().await;
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
                "peer handshake rejected seed={} node_id={} domain_hi=0x{:02X} reason={}",
                seed_key, remote.node.node_id, remote.cluster.domain_hi, reason
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
        info!(
            target: "pwmd::peer",
            "peer handshake completed seed={} node_id={} domain_hi=0x{:02X}",
            seed_key, remote.node.node_id, remote.cluster.domain_hi
        );
        info!(
            target: "pwmd::peer",
            "peer session open seed={} node_id={} domain_hi=0x{:02X}",
            seed_key, remote.node.node_id, remote.cluster.domain_hi
        );
    }
    crate::federation::merge_remote_hello(&app, &remote, now_ms).await;
    Some(remote)
}
