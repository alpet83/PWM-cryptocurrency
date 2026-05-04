//! Peer TCP session: wire framing, inbound acceptor path, outbound seed dial path.

use super::*;

mod inbound;
mod seed;
mod wire;

pub(super) use inbound::process_inbound_socket;
pub(super) use seed::run_seed_session;
#[allow(unused_imports)]
pub(super) use wire::decode_wire_msg_payload;
pub(super) use wire::{read_wire_msg, write_wire_msg, PeerWireMsg};

async fn peer_heartbeat_wire(app: &App, unix_ms: u64) -> PeerWireMsg {
    let (chain_tip_height, federation_gossip) = {
        let g = app.inner.read().await;
        let gossip = g.federation.gossip_wire_rows(unix_ms);
        (
            Some(g.chain.tip_h()),
            if gossip.is_empty() {
                None
            } else {
                Some(gossip)
            },
        )
    };
    PeerWireMsg::Heartbeat {
        unix_ms,
        chain_tip_height,
        federation_shard_id: Some(crate::runtime_shard_label(&app.identity, app.shard)),
        federation_gossip,
    }
}

async fn send_cross_shard_facts(
    app: &App,
    cfg: &TransportConfig,
    stream: &mut tokio::net::TcpStream,
) -> Result<(), String> {
    let facts = {
        let g = app.inner.read().await;
        g.cross_shard.facts()
    };
    write_wire_msg(
        stream,
        &PeerWireMsg::CrossShardFacts { facts },
        cfg.heartbeat_timeout_ms,
    )
    .await
}

async fn send_account_views(
    app: &App,
    cfg: &TransportConfig,
    stream: &mut tokio::net::TcpStream,
) -> Result<(), String> {
    let observed_at_ms = current_time_ms().unwrap_or(0);
    let rows = {
        let g = app.inner.read().await;
        g.local_account_views(app.identity.cluster_domain_hi, observed_at_ms)
    };
    write_wire_msg(
        stream,
        &PeerWireMsg::AccountViews { rows },
        cfg.heartbeat_timeout_ms,
    )
    .await
}

async fn merge_cross_shard_facts(
    app: &App,
    facts: Vec<crate::ledger::CrossShardFact>,
    trusted: bool,
) {
    if !trusted {
        return;
    }
    let mut g = app.inner.write().await;
    let changed = g.merge_cross_shard_facts(facts);
    if changed > 0 {
        info!(target: "pwmd::peer", "peer cross-shard facts merged count={changed}");
    }
}

async fn merge_account_views(
    app: &App,
    rows: Vec<crate::state::PeerAccountViewWire>,
    trusted: bool,
    source_node_id: &str,
    expected_domain_hi: u8,
    observed_at_ms: u64,
) {
    if !trusted {
        return;
    }
    let mut g = app.inner.write().await;
    let changed = g.merge_peer_acct_views(rows, source_node_id, expected_domain_hi);
    drop(g);
    let mut hs = app.handshake.write().await;
    if changed > 0 {
        let prev = hs.peer_merge_logged.get(source_node_id).copied();
        if prev != Some(changed) {
            info!(
                target: "pwmd::peer",
                "peer account views merged count={changed} source={source_node_id}"
            );
            hs.peer_merge_logged
                .insert(source_node_id.to_string(), changed);
        }
    }
    hs.trusted_account_streams.insert(
        source_node_id.to_string(),
        TrustedAccountStreamState {
            node_id: source_node_id.to_string(),
            domain_hi: expected_domain_hi,
            last_update_ms: observed_at_ms,
        },
    );
}

fn sticky_session_window_ms(cfg: &TransportConfig) -> u64 {
    cfg.heartbeat_timeout_ms
        .saturating_mul(2)
        .max(cfg.heartbeat_interval_ms.saturating_mul(4))
        .max(500)
}

fn has_sticky_trusted_session(
    hs: &HandshakeState,
    seed_key: &str,
    now_ms: u64,
    sticky_window_ms: u64,
) -> bool {
    let Some(node_id) = hs
        .transport
        .seed_peers
        .get(seed_key)
        .and_then(|x| x.last_node_id.as_ref())
    else {
        return false;
    };
    if !hs.trusted_peers.contains_key(node_id) {
        return false;
    }
    let Some(peer) = hs.peers.get(node_id) else {
        return false;
    };
    if !is_peer_liveish(&peer.status) {
        return false;
    }
    now_ms.saturating_sub(peer.last_seen_ms) <= sticky_window_ms
}

fn mark_trusted_peer_live(hs: &mut HandshakeState, node_id: &str, now_ms: u64) {
    if let Some(peer) = hs.peers.get_mut(node_id) {
        peer.status = PeerStatus::Connected;
        peer.last_seen_ms = now_ms;
    }
}

pub(super) fn peer_retry_sleep_ms(cfg: &TransportConfig, seed_key: &str, now_ms: u64) -> u64 {
    let jitter_window = cfg.retry_base_ms.max(50) / 4;
    let jitter = deterministic_seed_jitter_ms(seed_key, now_ms, jitter_window);
    cfg.retry_base_ms
        .saturating_add(jitter)
        .min(cfg.retry_max_ms)
        .max(200)
}

pub(super) fn deterministic_seed_jitter_ms(seed_key: &str, now_ms: u64, window_ms: u64) -> u64 {
    if window_ms == 0 {
        return 0;
    }
    let mut hash: u64 = 14695981039346656037;
    for b in seed_key.as_bytes() {
        hash ^= *b as u64;
        hash = hash.wrapping_mul(1099511628211);
    }
    let mixed = hash ^ now_ms.rotate_left(17);
    mixed % (window_ms.saturating_add(1))
}
