//! Peer hello and dev stats.

use super::common::ensure_ready;
use super::types::{PeerHelloOut, PeerStatsOut};
use crate::handshake::NodeHello;
use crate::transport::{
    count_native_live_peers, handshake_read_traced, handshake_write_traced, increment_class_bucket,
    is_peer_liveish, prioritize_peer_candidates_scored, process_incoming_peer_hello,
    SoakConfidenceSnapshot,
};
use crate::App;
use axum::{extract::State, http::StatusCode, Json};
use std::collections::HashMap;
use tracing::warn;

pub(super) async fn v1_peer_hello(
    State(a): State<App>,
    Json(hello): Json<NodeHello>,
) -> Result<Json<PeerHelloOut>, (StatusCode, String)> {
    ensure_ready(&a).await?;
    let transport_enabled = a.transport_config.read().await.enabled;
    if !a.dev_profile && !transport_enabled {
        return Err((
            StatusCode::NOT_FOUND,
            "peer hello endpoint is available only in dev profile or real transport mode"
                .to_string(),
        ));
    }
    let now_ms = crate::current_time_ms()?;
    let genesis_hash = {
        let hs = handshake_read_traced(&a, "api_handlers_peer").await;
        hs.validation_ctx.expected_genesis_hash.clone()
    };
    let chain_tip_height = {
        let g = a.inner.read().await;
        Some(g.chain.tip_h())
    };
    let bridge_commitment = crate::bridge_trust::local_bridge_commitment(&a).await;
    let local_hello = crate::transport::build_local_node_hello(
        &a,
        genesis_hash,
        Some(bridge_commitment.clone()),
        now_ms,
        chain_tip_height,
    );
    let mut hs = handshake_write_traced(&a, "api_handlers_peer").await;
    match process_incoming_peer_hello(
        &mut hs,
        &hello,
        now_ms,
        "http",
        false,
        Some(bridge_commitment.as_str()),
        a.identity.cluster_id.as_str(),
    ) {
        Ok(class) => Ok(Json(PeerHelloOut {
            accepted: true,
            reason: None,
            class: Some(class),
            node_hello: Some(local_hello),
        })),
        Err(label) => {
            warn!(
                target: "pwmd::peer",
                "peer hello http rejected reason={}",
                label
            );
            Ok(Json(PeerHelloOut {
                accepted: false,
                reason: Some(label),
                class: None,
                node_hello: None,
            }))
        }
    }
}

pub(super) async fn v1_dev_peers(
    State(a): State<App>,
) -> Result<Json<PeerStatsOut>, (StatusCode, String)> {
    ensure_ready(&a).await?;
    let transport_enabled = a.transport_config.read().await.enabled;
    if !a.dev_profile && !transport_enabled {
        return Err((
            StatusCode::NOT_FOUND,
            "peer stats endpoint is available only in dev profile or real transport mode"
                .to_string(),
        ));
    }
    let mut hs = handshake_write_traced(&a, "api_handlers_peer").await;
    hs.policy.counters.prioritize_runs += 1;
    let mut connected_by_class: HashMap<String, u64> = HashMap::new();
    for peer in hs.peers.values() {
        if is_peer_liveish(&peer.status) {
            increment_class_bucket(&mut connected_by_class, &peer.class);
        }
    }
    let native_live = count_native_live_peers(&hs);
    crate::transport::refresh_native_health(&mut hs, native_live, false);
    let peers = prioritize_peer_candidates_scored(hs.local_domain_hi, &hs.peers, &hs.peer_scores);
    Ok(Json(PeerStatsOut {
        accepted_total: hs.metrics.accepted_total,
        rejected_total: hs.metrics.rejected_total,
        reject_reason_total: hs.metrics.reject_reason_total.clone(),
        class_accept_total: hs.metrics.class_accept_total.clone(),
        connected_by_class,
        peers,
        policy: hs.policy.clone(),
        transport: hs.transport.snapshot.clone(),
        churn: hs.churn.clone(),
        soak: SoakConfidenceSnapshot {
            loop_ticks_capped: hs.transport.snapshot.soak_ticks_capped,
            stable_ticks_capped: hs
                .churn
                .stable_tick_total
                .min(hs.transport.snapshot.soak_ticks_capped),
            unstable_ticks_capped: hs
                .churn
                .unstable_tick_total
                .min(hs.transport.snapshot.soak_ticks_capped),
            reconnect_streak_current: hs.churn.reconnect_streak_current,
            reconnect_streak_max: hs.churn.reconnect_streak_max,
            runaway_stop_total: hs.transport.snapshot.reconnect_runaway_stop_total,
            runaway_guard_active: hs.transport.snapshot.reconnect_runaway_guard_active,
            health_snapshot_total: hs.transport.snapshot.soak_health_snapshot_total,
            health_last_tick: hs.transport.snapshot.soak_health_last_tick,
        },
    }))
}
