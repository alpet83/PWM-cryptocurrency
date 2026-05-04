//! Test helper forcing trusted handshake acceptance for integration paths.

use super::{build_local_node_hello, process_incoming_peer_hello, App};

pub(crate) async fn trust_peer_for_test(target: &App, source: &App) {
    let now_ms = crate::current_time_ms().expect("clock");
    let genesis_hash = {
        let hs = target.handshake.read().await;
        hs.validation_ctx.expected_genesis_hash.clone()
    };
    let chain_tip_height = {
        let g = source.inner.read().await;
        Some(g.chain.tip_h())
    };
    let bridge_commitment = crate::bridge_trust::local_bridge_commitment(source).await;
    let hello = build_local_node_hello(
        source,
        genesis_hash,
        Some(bridge_commitment.clone()),
        now_ms,
        chain_tip_height,
    );
    let mut hs = target.handshake.write().await;
    process_incoming_peer_hello(
        &mut hs,
        &hello,
        now_ms,
        "test-configured-seed",
        true,
        Some(bridge_commitment.as_str()),
    )
    .expect("trusted source hello");
    drop(hs);
    crate::federation::merge_remote_hello(target, &hello, now_ms).await;
}
