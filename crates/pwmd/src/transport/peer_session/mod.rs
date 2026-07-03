//! Peer TCP session: wire framing, inbound acceptor path, outbound seed dial path.

use super::*;
use crate::block_timing;
use crate::debug_dump::{dump_blk_json, DumpWrite};
use crate::handshake::{ClusterRole, NodeHello};
use pwm_core::block::{hdr_hash, Block};
use pwm_core::chain::{pick_prod_idx, roll_epoch_if_needed};
use pwm_core::{validate_tx_shape, SignedTx};

const SYNC_TX_OUT_CAP: usize = 32;
const SYNC_TX_IN_CAP: usize = 64;
const SYNC_TX_SCAN_CAP: usize = 256;
const SYNC_TX_RECENT_MS: u64 = 30_000;
const SYNC_TIP_COOLDOWN_MS: u64 = 60_000;
const CLUSTER_PROP_TAIL_CAP: usize = 10;

mod inbound;
mod seed;
mod sync_live;
mod wire;

use crate::transport::local_hello_signing_key;
pub(super) use inbound::process_inbound_socket;
pub(super) use seed::run_seed_session;
#[allow(unused_imports)]
pub(super) use wire::decode_wire_msg_payload;
pub(super) use wire::{
    read_wire_msg, write_wire_msg, ClusterAttestWire, ClusterProposeWire, PeerWireMsg,
    SyncBlockWire, SyncCatchupChunkWire, SyncHeaderWire, SyncWireHdr,
};

pub(crate) struct HandshakeWriteTraceGuard<'a> {
    tag: &'static str,
    acquired_ms: u64,
    guard: tokio::sync::RwLockWriteGuard<'a, HandshakeState>,
}

pub(crate) struct HandshakeReadTraceGuard<'a> {
    tag: &'static str,
    acquired_ms: u64,
    guard: tokio::sync::RwLockReadGuard<'a, HandshakeState>,
}

impl std::ops::Deref for HandshakeWriteTraceGuard<'_> {
    type Target = HandshakeState;

    fn deref(&self) -> &Self::Target {
        &self.guard
    }
}

impl std::ops::DerefMut for HandshakeWriteTraceGuard<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.guard
    }
}

impl std::ops::Deref for HandshakeReadTraceGuard<'_> {
    type Target = HandshakeState;

    fn deref(&self) -> &Self::Target {
        &self.guard
    }
}

impl Drop for HandshakeWriteTraceGuard<'_> {
    fn drop(&mut self) {
        let release_ms = current_time_ms().unwrap_or(self.acquired_ms);
        let held_ms = release_ms.saturating_sub(self.acquired_ms);
        if held_ms >= 100 {
            warn!(
                target: "pwmd::peer",
                "peer handshake lock_released tag={} ts_ms={} held_ms={}",
                self.tag,
                release_ms,
                held_ms
            );
        }
    }
}

impl Drop for HandshakeReadTraceGuard<'_> {
    fn drop(&mut self) {
        let release_ms = current_time_ms().unwrap_or(self.acquired_ms);
        let held_ms = release_ms.saturating_sub(self.acquired_ms);
        if held_ms >= 100 {
            warn!(
                target: "pwmd::peer",
                "peer handshake read_lock_released tag={} ts_ms={} held_ms={}",
                self.tag,
                release_ms,
                held_ms
            );
        }
    }
}

pub(crate) async fn handshake_write_traced<'a>(
    app: &'a App,
    tag: &'static str,
) -> HandshakeWriteTraceGuard<'a> {
    let wait_start_ms = current_time_ms().unwrap_or(0);
    let guard = app.handshake.write().await;
    let acquired_ms = current_time_ms().unwrap_or(wait_start_ms);
    let wait_ms = acquired_ms.saturating_sub(wait_start_ms);
    if wait_ms >= 100 {
        warn!(
            target: "pwmd::peer",
            "peer handshake lock_acquired tag={} ts_ms={} wait_ms={}",
            tag,
            acquired_ms,
            wait_ms
        );
    }
    HandshakeWriteTraceGuard {
        tag,
        acquired_ms,
        guard,
    }
}

pub(crate) async fn handshake_read_traced<'a>(
    app: &'a App,
    tag: &'static str,
) -> HandshakeReadTraceGuard<'a> {
    let wait_start_ms = current_time_ms().unwrap_or(0);
    let guard = app.handshake.read().await;
    let acquired_ms = current_time_ms().unwrap_or(wait_start_ms);
    let wait_ms = acquired_ms.saturating_sub(wait_start_ms);
    if wait_ms >= 100 {
        warn!(
            target: "pwmd::peer",
            "peer handshake read_lock_acquired tag={} ts_ms={} wait_ms={}",
            tag,
            acquired_ms,
            wait_ms
        );
    }
    HandshakeReadTraceGuard {
        tag,
        acquired_ms,
        guard,
    }
}

async fn peer_heartbeat_wire(app: &App, unix_ms: u64) -> PeerWireMsg {
    let (
        chain_tip_height,
        federation_gossip,
        lease_owner_id,
        lease_term,
        lease_expires_at_ms,
        lease_last_tip,
        lease_fence,
    ) = {
        let g = app.inner.read().await;
        let gossip = g.federation.gossip_wire_rows(unix_ms);
        let lease = app.lease_runtime.lock().ok().map(|x| x.clone());
        (
            Some(g.chain.tip_h()),
            if gossip.is_empty() {
                None
            } else {
                Some(gossip)
            },
            lease.as_ref().map(|x| x.owner_id.clone()),
            lease.as_ref().map(|x| x.term),
            lease.as_ref().map(|x| x.expires_at_ms),
            lease.as_ref().map(|x| x.last_tip),
            lease.as_ref().map(|x| x.fence),
        )
    };
    PeerWireMsg::Heartbeat {
        unix_ms,
        chain_tip_height,
        lease_owner_id,
        lease_term,
        lease_expires_at_ms,
        lease_last_tip,
        lease_fence,
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

fn tx_hash_hex(tx: &SignedTx) -> String {
    hex::encode(tx.tx_hash())
}

fn add_bucket(map: &mut HashMap<String, u64>, key: &str, delta: u64) {
    if delta == 0 {
        return;
    }
    if let Some(v) = map.get_mut(key) {
        *v = v.saturating_add(delta);
    } else {
        map.insert(key.to_string(), delta);
    }
}

fn sync_tx_items(msg: &PeerWireMsg) -> u64 {
    match msg {
        PeerWireMsg::SyncTxAnnounce { tx_ids, .. } | PeerWireMsg::SyncTxReq { tx_ids, .. } => {
            tx_ids.len() as u64
        }
        PeerWireMsg::SyncTxBatch { txs, .. } => txs.len() as u64,
        _ => 0,
    }
}

fn is_sync_tx_msg(msg: &PeerWireMsg) -> bool {
    matches!(
        msg,
        PeerWireMsg::SyncTxAnnounce { .. }
            | PeerWireMsg::SyncTxReq { .. }
            | PeerWireMsg::SyncTxBatch { .. }
    )
}

fn prune_mempool_gsp(hs: &mut HandshakeState, now_ms: u64) {
    hs.mempool_gsp
        .tx_seen_ms
        .retain(|_, ts| now_ms.saturating_sub(*ts) <= SYNC_TX_RECENT_MS);
    hs.mempool_gsp.tx_sent_peer_ms.retain(|_, sent_map| {
        sent_map.retain(|_, ts| now_ms.saturating_sub(*ts) <= SYNC_TX_RECENT_MS);
        !sent_map.is_empty()
    });
}

async fn add_sync_tx_drop(app: &App, reason: &str, count: u64) {
    if count == 0 {
        return;
    }
    let mut hs = handshake_write_traced(&app, "cluster_route_stub").await;
    hs.transport.snapshot.sync_tx_drop_total = hs
        .transport
        .snapshot
        .sync_tx_drop_total
        .saturating_add(count);
    increment_string_u64_bucket(&mut hs.transport.snapshot.sync_tx_drop_reason, reason);
}

async fn add_sync_v1_drop(app: &App, reason: &str) {
    let mut hs = handshake_write_traced(&app, "cluster_route_stub").await;
    hs.transport.snapshot.sync_v1_drop_total =
        hs.transport.snapshot.sync_v1_drop_total.saturating_add(1);
    increment_string_u64_bucket(&mut hs.transport.snapshot.sync_v1_drop_reason, reason);
}

async fn ingest_sync_tx_batch(app: &App, node_id: &str, mut txs: Vec<SignedTx>, cap_hint: usize) {
    let now_ms = current_time_ms().unwrap_or(0);
    let cap = cap_hint.min(SYNC_TX_IN_CAP).max(1);
    let seen = txs.len() as u64;
    if seen > 0 {
        let mut hs = handshake_write_traced(&app, "cluster_route_stub").await;
        add_bucket(
            &mut hs.transport.snapshot.mempool_ingress_kind_total,
            "p2p",
            seen,
        );
    }
    if txs.len() > cap {
        add_sync_tx_drop(app, "rate_limit", seen).await;
        warn!(
            target: "pwmd::peer",
            "peer sync tx batch dropped node_id={} reason=rate_limit got={} cap={}",
            node_id,
            txs.len(),
            cap
        );
        return;
    }
    let mut accepted = 0u64;
    let mut dropped_dup = 0u64;
    let mut dropped_invalid = 0u64;
    for tx in txs.drain(..) {
        let tx_id = tx_hash_hex(&tx);
        {
            let mut hs = handshake_write_traced(&app, "cluster_route_stub").await;
            prune_mempool_gsp(&mut hs, now_ms);
            if hs.mempool_gsp.tx_seen_ms.contains_key(&tx_id) {
                dropped_dup = dropped_dup.saturating_add(1);
                continue;
            }
            hs.mempool_gsp.tx_seen_ms.insert(tx_id.clone(), now_ms);
        }
        if validate_tx_shape(&tx).is_err() {
            dropped_invalid = dropped_invalid.saturating_add(1);
            continue;
        }
        let mut g = app.inner.write().await;
        let Ok((next_h, next_ts)) = g.chain.next_apply_ctx() else {
            dropped_invalid = dropped_invalid.saturating_add(1);
            continue;
        };
        if g.chain
            .st
            .precheck_apply_with_ctx(&tx, next_h, next_ts, &g.chain.cfg)
            .is_err()
        {
            dropped_invalid = dropped_invalid.saturating_add(1);
            continue;
        }
        if g.pool.push(tx).is_err() {
            dropped_invalid = dropped_invalid.saturating_add(1);
            continue;
        }
        accepted = accepted.saturating_add(1);
    }
    if accepted > 0 {
        let mut hs = handshake_write_traced(&app, "cluster_route_stub").await;
        hs.transport.snapshot.sync_tx_accept_total = hs
            .transport
            .snapshot
            .sync_tx_accept_total
            .saturating_add(accepted);
    }
    add_sync_tx_drop(app, "duplicate", dropped_dup).await;
    add_sync_tx_drop(app, "invalid", dropped_invalid).await;
    info!(
        target: "pwmd::peer",
        "peer sync tx batch handled node_id={} seen={} accepted={} dropped_dup={} dropped_invalid={}",
        node_id,
        seen,
        accepted,
        dropped_dup,
        dropped_invalid
    );
}

async fn send_sync_tx_batch(
    app: &App,
    cfg: &TransportConfig,
    stream: &mut tokio::net::TcpStream,
    remote: &NodeHello,
    seq_no: &mut u64,
) -> Result<(), String> {
    if remote.cluster.domain_hi != app.identity.cluster_domain_hi {
        return Ok(());
    }
    if !remote.capabilities.supports_sync_v1() {
        return Ok(());
    }
    let profile_cap = remote
        .capabilities
        .sync_profile
        .as_ref()
        .map(|x| x.max_txs_per_msg as usize)
        .unwrap_or(0);
    if profile_cap == 0 {
        return Ok(());
    }
    let cap = profile_cap.min(SYNC_TX_OUT_CAP);
    let now_ms = current_time_ms().unwrap_or(0);
    let txs = {
        let g = app.inner.read().await;
        g.pool.snapshot(SYNC_TX_SCAN_CAP)
    };
    if txs.is_empty() {
        return Ok(());
    }
    let mut out = Vec::new();
    let mut suppressed = 0u64;
    {
        let mut hs = handshake_write_traced(&app, "cluster_route_stub").await;
        prune_mempool_gsp(&mut hs, now_ms);
        let sent = hs
            .mempool_gsp
            .tx_sent_peer_ms
            .entry(remote.node.node_id.clone())
            .or_default();
        for tx in txs.into_iter() {
            if out.len() >= cap {
                break;
            }
            let tx_id = tx_hash_hex(&tx);
            if sent
                .get(&tx_id)
                .map(|ts| now_ms.saturating_sub(*ts) <= SYNC_TX_RECENT_MS)
                .unwrap_or(false)
            {
                suppressed = suppressed.saturating_add(1);
                continue;
            }
            out.push(tx);
        }
    }
    if suppressed > 0 {
        let mut hs = handshake_write_traced(&app, "cluster_route_stub").await;
        add_bucket(
            &mut hs.transport.snapshot.mempool_push_suppressed,
            "recent_peer_dedup",
            suppressed,
        );
        info!(
            target: "pwmd::peer",
            "peer storm guard suppress node_id={} reason=recent_peer_dedup count={}",
            remote.node.node_id,
            suppressed
        );
    }
    if out.is_empty() {
        return Ok(());
    }
    *seq_no = seq_no.saturating_add(1);
    let hdr = SyncWireHdr {
        shard_id: app.identity.cluster_domain_hi,
        peer_session_id: app.identity.node_id.clone(),
        seq_no: *seq_no,
        timestamp_ms: now_ms,
    };
    write_wire_msg(
        stream,
        &PeerWireMsg::SyncTxBatch {
            hdr,
            txs: out.clone(),
        },
        cfg.heartbeat_timeout_ms,
    )
    .await?;
    let out_len = out.len() as u64;
    {
        let mut hs = handshake_write_traced(&app, "cluster_route_stub").await;
        let sent = hs
            .mempool_gsp
            .tx_sent_peer_ms
            .entry(remote.node.node_id.clone())
            .or_default();
        for tx in out.into_iter() {
            sent.insert(tx_hash_hex(&tx), now_ms);
        }
        add_bucket(
            &mut hs.transport.snapshot.mempool_egress_relay_total,
            "same_shard_peer",
            out_len,
        );
    }
    info!(
        target: "pwmd::peer",
        "peer storm guard egress route node_id={} class=same_shard_peer txs={}",
        remote.node.node_id,
        out_len
    );
    Ok(())
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
    let mut hs = handshake_write_traced(&app, "cluster_route_stub").await;
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

/// Attest signature domain (RFC §5 binding): vote object, candidate hash, and optional `candidate_ref` (VO2).
fn cluster_sig_msg(
    height: u64,
    round: u32,
    vote: &str,
    cand: &str,
    candidate_ref: Option<&str>,
) -> Vec<u8> {
    let cref = candidate_ref.unwrap_or("");
    format!("{height}\n{round}\n{vote}\n{cand}\n{cref}").into_bytes()
}

fn verify_attest_sig(sig_hex: &str, pubkey: &[u8; 32], msg: &[u8]) -> bool {
    let Ok(sig_raw) = hex::decode(sig_hex.trim()) else {
        return false;
    };
    if sig_raw.len() != 64 {
        return false;
    }
    let mut sig = [0u8; 64];
    sig.copy_from_slice(&sig_raw);
    pwm_core::crypto::verify(pubkey, msg, &sig)
}

fn peer_member_id(peer: &TrustedPeer) -> Option<&str> {
    peer.instance_id
        .as_deref()
        .map(str::trim)
        .filter(|id| !id.is_empty())
}

fn node_member_id(remote: &NodeHello) -> Option<&str> {
    remote
        .capabilities
        .node_instance_id
        .as_deref()
        .map(str::trim)
        .filter(|id| !id.is_empty())
}

fn cluster_role_ok(local: ClusterRole, remote: ClusterRole, propose_frame: bool) -> bool {
    match (local, remote, propose_frame) {
        (ClusterRole::Attester, ClusterRole::Proposer, true) => true,
        (ClusterRole::Proposer, ClusterRole::Attester, false) => true,
        _ => false,
    }
}

fn record_cluster_prop(
    hs: &mut HandshakeState,
    msg: &ClusterProposeWire,
    proposer_member_id: &str,
    opened_at_ms: Option<u64>,
) -> usize {
    let entry = hs
        .cluster_attest
        .rounds
        .entry((msg.height, msg.round))
        .or_default();
    entry.vote_object = msg.vote_object.clone();
    entry.candidate_hash = msg.candidate_hash.clone();
    entry.candidate_ref = msg.candidate_ref.clone();
    entry.proposer_id = Some(proposer_member_id.to_string());
    let bind_changed = entry.vote_object != msg.vote_object
        || entry.candidate_hash != msg.candidate_hash
        || entry.candidate_ref != msg.candidate_ref
        || entry
            .proposer_id
            .as_deref()
            .is_some_and(|x| x != proposer_member_id);
    if bind_changed {
        entry.attesters.clear();
        entry.propose_retry_n = 0;
        entry.propose_opened_at_ms = opened_at_ms;
    } else if entry.propose_opened_at_ms.is_none() {
        entry.propose_opened_at_ms = opened_at_ms;
    }
    entry.attesters.len()
}

pub(crate) async fn record_cluster_propose_originated(
    app: &App,
    msg: ClusterProposeWire,
    proposer_member_id: &str,
    opened_at_ms: Option<u64>,
) {
    let mut hs = handshake_write_traced(&app, "cluster_route_stub").await;
    let _ = record_cluster_prop(&mut hs, &msg, proposer_member_id, opened_at_ms);
}

pub(crate) async fn record_cluster_prop_tick(app: &App) {
    let Some(msg) = mk_cluster_prop(app, None).await else {
        return;
    };
    let local_member = app.node_instance_id.trim().to_string();
    // Local seal/ahead tick may open round binding, but timeout anchor must wait
    // for confirmed wire send to at least one attester.
    record_cluster_propose_originated(app, msg, &local_member, None).await;
}

pub(crate) async fn mark_cluster_prop_opened(app: &App, h: u64, r: u32, opened_ms: u64) {
    let mut hs = handshake_write_traced(&app, "cluster_route_stub").await;
    let Some(round) = hs.cluster_attest.rounds.get_mut(&(h, r)) else {
        return;
    };
    if round.propose_opened_at_ms.is_none() {
        round.propose_opened_at_ms = Some(opened_ms);
    }
}

pub(crate) async fn maybe_retry_round(app: &App, h: u64, r: u32, max_retry: u8) -> Option<u8> {
    let mut hs = handshake_write_traced(&app, "cluster_route_stub").await;
    let (retry_n, can_retry) = {
        let round = hs.cluster_attest.rounds.get_mut(&(h, r))?;
        if !round.attesters.is_empty() || round.propose_retry_n >= max_retry {
            (round.propose_retry_n, false)
        } else {
            round.propose_opened_at_ms = None;
            round.propose_retry_n = round.propose_retry_n.saturating_add(1);
            (round.propose_retry_n, true)
        }
    };
    if !can_retry {
        return None;
    }
    hs.cluster_attest
        .sent_key_by_node
        .retain(|_, k| *k != (h, r));
    Some(retry_n)
}

async fn mk_cluster_prop(app: &App, remote_tip_h: Option<u64>) -> Option<ClusterProposeWire> {
    if !app.cluster_cfg.enabled || app.cluster_cfg.role != ClusterRole::Proposer {
        return None;
    }
    let proposer = app.node_instance_id.trim();
    if proposer.is_empty() || !app.cluster_cfg.members.iter().any(|x| x == proposer) {
        return None;
    }
    let manual_target_h = {
        let manual = app.seal_manual.read().await;
        if manual.mode.is_manual_rpc() {
            manual.target_h
        } else {
            0
        }
    };
    let g = app.inner.read().await;
    let tip_h = g.chain.tip_h();
    let height = manual_target_h.max(tip_h.saturating_add(1));
    let active_idxs =
        if g.chain.cfg.epoch_length_blocks > 0 && height % g.chain.cfg.epoch_length_blocks == 0 {
            let mut st = g.chain.st.clone();
            roll_epoch_if_needed(&g.chain.cfg, &mut st, height);
            st.active_validator_indices
        } else {
            g.chain.st.active_validator_indices.clone()
        };
    let prod_idx = pick_prod_idx(height, &active_idxs).ok()? as usize;
    if app.cluster_cfg.members.get(prod_idx).map(String::as_str) != Some(proposer) {
        return None;
    }
    let tip_hash = hex::encode(g.chain.tip_hash());
    let vote = format!("vo1:{height}:{tip_hash}");
    let mut tail_blocks: Vec<SyncBlockWire> = if app.cluster_cfg.full_blocks {
        let gap = remote_tip_h.map(|h| tip_h.saturating_sub(h)).unwrap_or(0);
        let tail_depth = usize::try_from(gap.saturating_add(2))
            .unwrap_or(CLUSTER_PROP_TAIL_CAP)
            .clamp(1, CLUSTER_PROP_TAIL_CAP);
        g.chain
            .blocks
            .iter()
            .rev()
            .take(tail_depth)
            .map(|blk| SyncBlockWire {
                height: blk.hdr.height,
                hash: hex::encode(hdr_hash(&blk.hdr)),
                block: Some(blk.clone()),
            })
            .collect()
    } else {
        Vec::new()
    };
    tail_blocks.reverse();
    Some(ClusterProposeWire {
        height,
        round: 0,
        vote_object: vote,
        candidate_hash: tip_hash,
        candidate_ref: None,
        tail_blocks,
    })
}

pub(super) async fn send_cluster_prop(
    app: &App,
    cfg: &TransportConfig,
    stream: &mut tokio::net::TcpStream,
    remote: &NodeHello,
) -> Result<(), String> {
    let Some(remote_member) = node_member_id(remote) else {
        return Ok(());
    };
    if !app.cluster_cfg.members.iter().any(|x| x == remote_member) {
        return Ok(());
    }
    if app.cluster_cfg.role != ClusterRole::Proposer
        || remote.capabilities.cluster_role != ClusterRole::Attester
    {
        return Ok(());
    }
    let remote_tip_h = {
        let hs = handshake_read_traced(&app, "peer_session_mod").await;
        hs.sync_live
            .peers
            .get(&remote.node.node_id)
            .map(|x| x.tip_h)
            .filter(|h| *h > 0)
    };
    let Some(msg) = mk_cluster_prop(app, remote_tip_h).await else {
        return Ok(());
    };
    {
        let hs = handshake_read_traced(&app, "peer_session_mod").await;
        if hs
            .cluster_attest
            .sent_key_by_node
            .get(&remote.node.node_id)
            .is_some_and(|k| *k == (msg.height, msg.round))
        {
            return Ok(());
        }
    }
    let local_member = app.node_instance_id.trim();
    write_wire_msg(
        stream,
        &PeerWireMsg::ClusterPropose { msg: msg.clone() },
        cfg.heartbeat_timeout_ms,
    )
    .await?;
    let opened_ms = crate::current_time_ms().unwrap_or(0);
    record_cluster_propose_originated(app, msg.clone(), local_member, Some(opened_ms)).await;
    mark_cluster_prop_opened(app, msg.height, msg.round, opened_ms).await;
    if let Some(bt) = app.block_timing.as_ref() {
        block_timing::note_send(
            bt,
            block_timing::SendCtx {
                h: msg.height,
                r: msg.round,
                t_ms: block_timing::now_ms_f64(),
            },
        );
    }
    {
        let mut hs = handshake_write_traced(&app, "cluster_route_stub").await;
        hs.cluster_attest
            .sent_key_by_node
            .insert(remote.node.node_id.clone(), (msg.height, msg.round));
    }
    info!(
        target: "pwmd::peer",
        "cluster propose sent node_id={} member_id={} height={} round={} remote_tip_h={:?} tail_blocks={}",
        remote.node.node_id,
        remote_member,
        msg.height,
        msg.round,
        remote_tip_h,
        msg.tail_blocks.len()
    );
    Ok(())
}

/// Seal-loop ahead-trigger: send wire propose before the next heartbeat sleep.
pub(super) async fn try_prop_nudge(
    app: &App,
    cfg: &TransportConfig,
    stream: &mut tokio::net::TcpStream,
    remote: &NodeHello,
) -> Result<(), String> {
    use std::sync::atomic::Ordering;
    if app.cluster_cfg.seal_ahead_ms == 0 {
        return Ok(());
    }
    if app.cluster_prop_nudge.swap(false, Ordering::AcqRel) {
        send_cluster_prop(app, cfg, stream, remote).await
    } else {
        Ok(())
    }
}

async fn mk_cluster_attest(app: &App, msg: &ClusterProposeWire) -> Option<ClusterAttestWire> {
    if app.cluster_cfg.role != ClusterRole::Attester {
        return None;
    }
    let local_member = app.node_instance_id.trim();
    if local_member.is_empty() || !app.cluster_cfg.members.iter().any(|x| x == local_member) {
        return None;
    }
    let sk = local_hello_signing_key(&app.identity);
    let sign_msg = cluster_sig_msg(
        msg.height,
        msg.round,
        &msg.vote_object,
        &msg.candidate_hash,
        msg.candidate_ref.as_deref(),
    );
    let attester_tip_height = {
        let g = app.inner.read().await;
        Some(g.chain.tip_h())
    };
    Some(ClusterAttestWire {
        height: msg.height,
        round: msg.round,
        vote_object: msg.vote_object.clone(),
        candidate_hash: msg.candidate_hash.clone(),
        signature: hex::encode(pwm_core::crypto::sign(&sk, &sign_msg)),
        candidate_ref: msg.candidate_ref.clone(),
        attester_tip_height,
    })
}

async fn route_cluster_stub(
    app: &App,
    node_id: &str,
    msg: PeerWireMsg,
) -> Option<ClusterAttestWire> {
    if !app.cluster_cfg.enabled {
        info!(
            target: "pwmd::peer",
            "cluster frame ignored node_id={} reason=cluster_disabled",
            node_id
        );
        return None;
    }
    let route_lock_wait_start_ms = crate::current_time_ms().unwrap_or(0);
    let mut hs = handshake_write_traced(&app, "cluster_route_stub").await;
    let route_lock_acquired_ms = crate::current_time_ms().unwrap_or(route_lock_wait_start_ms);
    if route_lock_acquired_ms.saturating_sub(route_lock_wait_start_ms) >= 100 {
        info!(
            target: "pwmd::peer",
            "cluster route step=state_lock_acquired node_id={} ts_ms={} wait_ms={}",
            node_id,
            route_lock_acquired_ms,
            route_lock_acquired_ms.saturating_sub(route_lock_wait_start_ms)
        );
    }
    let Some(peer) = hs.trusted_peers.get(node_id).cloned() else {
        warn!(
            target: "pwmd::peer",
            "cluster frame dropped node_id={} reason=untrusted_peer",
            node_id
        );
        return None;
    };
    if !peer.cluster_attest_enabled {
        warn!(
            target: "pwmd::peer",
            "cluster frame dropped node_id={} reason=peer_hello_cluster_disabled",
            node_id
        );
        return None;
    }
    let Some(member_id) = peer_member_id(&peer).map(str::to_string) else {
        warn!(
            target: "pwmd::peer",
            "cluster frame dropped node_id={} reason=peer_instance_missing",
            node_id
        );
        return None;
    };
    if !app.cluster_cfg.members.iter().any(|m| m == &member_id) {
        warn!(
            target: "pwmd::peer",
            "cluster frame dropped node_id={} member_id={} reason=non_member",
            node_id,
            member_id
        );
        return None;
    }
    match msg {
        PeerWireMsg::ClusterPropose {
            msg: msg @ ClusterProposeWire { height, round, .. },
        } => {
            if !cluster_role_ok(app.cluster_cfg.role, peer.cluster_role, true) {
                warn!(
                    target: "pwmd::peer",
                    "cluster propose dropped node_id={} member_id={} reason=peer_hello_role_mismatch local_role={:?} remote_role={:?}",
                    node_id,
                    member_id,
                    app.cluster_cfg.role,
                    peer.cluster_role
                );
                return None;
            }
            // Do not hold handshake write-lock while waiting on inner state lock.
            drop(hs);
            let local_tip_read_start_ms =
                crate::current_time_ms().unwrap_or(route_lock_acquired_ms);
            if local_tip_read_start_ms.saturating_sub(route_lock_acquired_ms) >= 100 {
                info!(
                    target: "pwmd::peer",
                    "cluster route step=before_local_tip_read node_id={} member_id={} ts_ms={} lock_held_ms={} height={} round={}",
                    node_id,
                    member_id,
                    local_tip_read_start_ms,
                    local_tip_read_start_ms.saturating_sub(route_lock_acquired_ms),
                    height,
                    round
                );
            }
            let local_tip = {
                let g = app.inner.read().await;
                g.chain.tip_h()
            };
            let local_tip_read_done_ms =
                crate::current_time_ms().unwrap_or(local_tip_read_start_ms);
            if local_tip_read_done_ms.saturating_sub(local_tip_read_start_ms) >= 100 {
                info!(
                    target: "pwmd::peer",
                    "cluster route step=after_local_tip_read node_id={} member_id={} ts_ms={} read_latency_ms={} lock_held_ms={} height={} round={} local_tip={}",
                    node_id,
                    member_id,
                    local_tip_read_done_ms,
                    local_tip_read_done_ms.saturating_sub(local_tip_read_start_ms),
                    local_tip_read_done_ms.saturating_sub(route_lock_acquired_ms),
                    height,
                    round,
                    local_tip
                );
            }
            hs = handshake_write_traced(&app, "cluster_route_stub").await;
            if local_tip.saturating_add(1) < height {
                let mut tail_batch: Vec<Block> = msg
                    .tail_blocks
                    .iter()
                    .filter_map(|x| x.block.clone())
                    .collect();
                tail_batch.sort_by_key(|b| b.hdr.height);
                tail_batch.retain(|b| b.hdr.height > local_tip);
                if !tail_batch.is_empty() {
                    drop(hs);
                    match sync_live::apply_cluster_tail_blocks(app, &tail_batch).await {
                        Ok(applied) => {
                            info!(
                                target: "pwmd::peer",
                                "cluster propose tail_applied node_id={} member_id={} local_tip={} target_height={} tail_rows={} applied={}",
                                node_id,
                                member_id,
                                local_tip,
                                height,
                                tail_batch.len(),
                                applied
                            );
                        }
                        Err(err) => {
                            warn!(
                                target: "pwmd::peer",
                                "cluster propose tail_apply_failed node_id={} member_id={} local_tip={} target_height={} tail_rows={} err={}",
                                node_id,
                                member_id,
                                local_tip,
                                height,
                                tail_batch.len(),
                                err
                            );
                        }
                    }
                    hs = handshake_write_traced(&app, "cluster_route_stub").await;
                }
            }
            let attesters_n = record_cluster_prop(
                &mut hs,
                &msg,
                &member_id,
                Some(crate::current_time_ms().unwrap_or(0)),
            );
            if let Some(bt) = app.block_timing.as_ref() {
                block_timing::note_att_rx(
                    bt,
                    block_timing::AttCtx {
                        h: height,
                        r: round,
                        t_ms: block_timing::now_ms_f64(),
                        att_id: app.node_instance_id.clone(),
                    },
                );
            }
            let proc_start_ms = block_timing::now_ms_f64();
            let proc_start_at = std::time::Instant::now();
            info!(
                target: "pwmd::peer",
                "cluster propose accepted node_id={} member_id={} height={} round={} attesters={} tail_blocks={}",
                node_id, member_id,
                height,
                round,
                attesters_n,
                msg.tail_blocks.len()
            );
            drop(hs);
            let out = mk_cluster_attest(app, &msg).await;
            let proc_ms = proc_start_at.elapsed().as_secs_f64() * 1000.0;
            info!(
                target: "pwmd::peer",
                "cluster_attest_proc h={} round={} proc_ms={:.2}",
                height, round, proc_ms
            );
            if let Some(bt) = app.block_timing.as_ref() {
                block_timing::note_att_proc(
                    bt,
                    block_timing::ProcCtx {
                        h: height,
                        r: round,
                        start_ms: proc_start_ms,
                        proc_ms,
                    },
                );
            }
            return out;
        }
        PeerWireMsg::ClusterAttest {
            msg:
                ClusterAttestWire {
                    height,
                    round,
                    vote_object,
                    candidate_hash,
                    signature,
                    candidate_ref,
                    attester_tip_height,
                },
        } => {
            if !cluster_role_ok(app.cluster_cfg.role, peer.cluster_role, false) {
                warn!(
                    target: "pwmd::peer",
                    "cluster attest dropped node_id={} member_id={} reason=peer_hello_role_mismatch local_role={:?} remote_role={:?}",
                    node_id,
                    member_id,
                    app.cluster_cfg.role,
                    peer.cluster_role
                );
                return None;
            }
            if let Some(tip_h) = attester_tip_height {
                let st = hs.sync_live.peers.entry(node_id.to_string()).or_default();
                st.tip_h = st.tip_h.max(tip_h);
            }
            if let Some(entry) = hs.cluster_attest.rounds.get_mut(&(height, round)) {
                if entry.vote_object == vote_object
                    && entry.candidate_hash == candidate_hash
                    && entry.candidate_ref == candidate_ref
                {
                    let msg = cluster_sig_msg(
                        height,
                        round,
                        &vote_object,
                        &candidate_hash,
                        candidate_ref.as_deref(),
                    );
                    if !verify_attest_sig(&signature, &peer.pubkey, &msg) {
                        warn!(
                            target: "pwmd::peer",
                            "cluster attest dropped node_id={} member_id={} reason=invalid_signature height={} round={}",
                            node_id,
                            member_id,
                            height,
                            round
                        );
                        return None;
                    }
                    entry.attesters.insert(member_id.clone(), signature);
                    if let Some(bt) = app.block_timing.as_ref() {
                        block_timing::note_att_ok(
                            bt,
                            block_timing::AttCtx {
                                h: height,
                                r: round,
                                t_ms: block_timing::now_ms_f64(),
                                att_id: member_id.clone(),
                            },
                        );
                    }
                    info!(
                        target: "pwmd::peer",
                        "cluster attest accepted node_id={} member_id={} height={} round={} attesters={} attester_tip_height={:?}",
                        node_id, member_id,
                        height,
                        round,
                        entry.attesters.len(),
                        attester_tip_height
                    );
                } else {
                    warn!(
                        target: "pwmd::peer",
                        "cluster attest dropped node_id={} member_id={} reason=binding_mismatch height={} round={}",
                        node_id,
                        member_id,
                        height,
                        round
                    );
                }
            } else {
                warn!(
                    target: "pwmd::peer",
                    "cluster attest dropped node_id={} member_id={} reason=missing_propose height={} round={}",
                    node_id,
                    member_id,
                    height,
                    round
                );
            }
        }
        _ => {}
    }
    None
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

fn seed_key_by_node(hs: &HandshakeState, node_id: &str) -> Option<String> {
    hs.transport.seed_peers.iter().find_map(|(seed_key, st)| {
        if st.last_node_id.as_deref() == Some(node_id) {
            Some(seed_key.clone())
        } else {
            None
        }
    })
}

fn mark_trusted_peer_live(hs: &mut HandshakeState, node_id: &str, now_ms: u64) {
    if let Some(peer) = hs.peers.get_mut(node_id) {
        peer.status = PeerStatus::Connected;
        peer.last_seen_ms = now_ms;
    }
}

fn peer_sync_v1(hello: &NodeHello) -> bool {
    hello.capabilities.supports_sync_v1()
}

fn sync_mode_text(hello: &NodeHello) -> &'static str {
    match hello.capabilities.sync_mode() {
        crate::handshake::SyncMode::FullV1 => "full_v1",
        crate::handshake::SyncMode::LegacyObserve => "legacy_observe",
    }
}

fn sync_wire_hdr(msg: &PeerWireMsg) -> Option<&SyncWireHdr> {
    match msg {
        PeerWireMsg::SyncProfileAnnounce { hdr, .. }
        | PeerWireMsg::SyncTipAnnounce { hdr, .. }
        | PeerWireMsg::SyncHeadersReq { hdr, .. }
        | PeerWireMsg::SyncHeadersBatch { hdr, .. }
        | PeerWireMsg::SyncBlocksReq { hdr, .. }
        | PeerWireMsg::SyncBlocksBatch { hdr, .. }
        | PeerWireMsg::SyncTxAnnounce { hdr, .. }
        | PeerWireMsg::SyncTxReq { hdr, .. }
        | PeerWireMsg::SyncTxBatch { hdr, .. }
        | PeerWireMsg::SyncNack { hdr, .. }
        | PeerWireMsg::SyncCatchupReq { hdr, .. }
        | PeerWireMsg::SyncCatchupChunk { hdr, .. }
        | PeerWireMsg::SyncCatchupDone { hdr, .. } => Some(hdr),
        _ => None,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum SyncRouteOutcome {
    Continue,
    Disconnect {
        reason: PeerCloseReason,
        detail: String,
    },
}

async fn route_sync_stub(
    app: &App,
    cfg: &TransportConfig,
    stream: &mut tokio::net::TcpStream,
    seed_key: Option<&str>,
    node_id: &str,
    msg: PeerWireMsg,
    full_v1: bool,
    local_domain_hi: u8,
    same_shard: bool,
    hdr_cap: u16,
    blk_cap: u16,
    can_cup: bool,
    seq_no: &mut u64,
) -> SyncRouteOutcome {
    let Some(hdr) = sync_wire_hdr(&msg).cloned() else {
        return SyncRouteOutcome::Continue;
    };
    let tx_items = sync_tx_items(&msg);
    let tx_frame = is_sync_tx_msg(&msg);
    {
        let mut hs = handshake_write_traced(&app, "cluster_route_stub").await;
        hs.transport.snapshot.sync_v1_seen_total =
            hs.transport.snapshot.sync_v1_seen_total.saturating_add(1);
        hs.peer_scores.apply(node_id, PeerScoreEvent::SyncRound);
        if tx_frame && tx_items > 0 {
            hs.transport.snapshot.sync_tx_seen_total = hs
                .transport
                .snapshot
                .sync_tx_seen_total
                .saturating_add(tx_items);
        }
    }
    if hdr.shard_id != local_domain_hi {
        add_sync_v1_drop(app, "shard_mismatch").await;
        warn!(
            target: "pwmd::peer",
            "peer sync frame dropped node_id={} reason=shard_mismatch local=0x{:02X} remote=0x{:02X}",
            node_id,
            local_domain_hi,
            hdr.shard_id
        );
        if tx_frame {
            add_sync_tx_drop(app, "shard_mismatch", tx_items.max(1)).await;
        }
        return SyncRouteOutcome::Continue;
    }
    if !same_shard {
        add_sync_v1_drop(app, "inter_shard_sync_forbidden").await;
        warn!(
            target: "pwmd::peer",
            "peer sync frame ignored node_id={} reason=inter_shard_sync_forbidden session_id={}",
            node_id,
            hdr.peer_session_id
        );
        if tx_frame {
            add_sync_tx_drop(app, "inter_shard_sync_forbidden", tx_items.max(1)).await;
        }
        return SyncRouteOutcome::Continue;
    }
    if !full_v1 {
        add_sync_v1_drop(app, "same_shard_profile_mismatch").await;
        warn!(
            target: "pwmd::peer",
            "peer sync frame ignored node_id={} reason=same_shard_profile_mismatch session_id={}",
            node_id,
            hdr.peer_session_id
        );
        if tx_frame {
            add_sync_tx_drop(app, "same_shard_profile_mismatch", tx_items.max(1)).await;
        }
        return SyncRouteOutcome::Continue;
    }
    match msg {
        PeerWireMsg::SyncTxBatch { txs, .. } => {
            ingest_sync_tx_batch(app, node_id, txs, SYNC_TX_IN_CAP).await;
        }
        PeerWireMsg::SyncTxAnnounce { tx_ids, .. } => {
            add_sync_tx_drop(app, "unsupported_msg", tx_ids.len() as u64).await;
            info!(
                target: "pwmd::peer",
                "peer sync tx announce ignored node_id={} count={}",
                node_id,
                tx_ids.len()
            );
        }
        PeerWireMsg::SyncTxReq { tx_ids, .. } => {
            add_sync_tx_drop(app, "unsupported_msg", tx_ids.len() as u64).await;
            info!(
                target: "pwmd::peer",
                "peer sync tx req ignored node_id={} count={}",
                node_id,
                tx_ids.len()
            );
        }
        PeerWireMsg::SyncTipAnnounce {
            head_height,
            head_hash,
            finalized_height,
            finalized_hash,
            ..
        } => {
            match sync_live::on_tip(
                app,
                cfg,
                stream,
                node_id,
                head_height,
                &head_hash,
                finalized_height,
                finalized_hash.as_deref(),
                hdr_cap,
                can_cup,
                seq_no,
            )
            .await
            {
                Ok(Some(div)) => {
                    let now_ms = current_time_ms().unwrap_or(0);
                    let cooldown_ms = cfg.reconnect_runaway_cooldown_ms.max(SYNC_TIP_COOLDOWN_MS);
                    let mut hs = handshake_write_traced(&app, "cluster_route_stub").await;
                    hs.transport.snapshot.sync_tip_disconnect_total = hs
                        .transport
                        .snapshot
                        .sync_tip_disconnect_total
                        .saturating_add(1);
                    let div_streak = {
                        hs.peer_scores.apply(node_id, PeerScoreEvent::ForkMismatch);
                        let st = hs.sync_live.peers.entry(node_id.to_string()).or_default();
                        st.div_streak = st.div_streak.saturating_add(1);
                        st.div_streak.max(1)
                    };
                    let cooldown_seed = seed_key
                        .map(str::to_string)
                        .or_else(|| seed_key_by_node(&hs, node_id));
                    if let Some(seed_key) = cooldown_seed.as_deref() {
                        set_seed_due(&mut hs, seed_key, now_ms.saturating_add(cooldown_ms));
                    }
                    warn!(
                        target: "pwmd::peer",
                        "peer sync divergence disconnect node_id={} local_height={} local_hash={} peer_height={} peer_hash={} cooldown_ms={} streak={}",
                        node_id,
                        div.local_h,
                        div.local_hash,
                        div.peer_h,
                        div.peer_hash,
                        cooldown_ms,
                        div_streak
                    );
                    drop(hs);
                    if app.debug_dump.on_divergence
                        && div_streak >= app.debug_dump.trigger_streak.max(2)
                    {
                        let blk_opt = {
                            let g = app.inner.read().await;
                            g.chain
                                .blocks
                                .iter()
                                .rev()
                                .find(|blk| {
                                    blk.hdr.height == div.local_h
                                        && hex::encode(hdr_hash(&blk.hdr)) == div.local_hash
                                })
                                .cloned()
                        };
                        if let Some(blk) = blk_opt {
                            match dump_blk_json(app, &blk, "divergence_probe", node_id) {
                                Ok(DumpWrite::Wrote(path)) => info!(
                                    target: "pwmd::peer",
                                    "divergence debug dump written node_id={} height={} path={}",
                                    node_id,
                                    blk.hdr.height,
                                    path.display()
                                ),
                                Ok(DumpWrite::CapReached) => warn!(
                                    target: "pwmd::peer",
                                    "divergence debug dump skipped node_id={} reason=cap_reached cap={}",
                                    node_id,
                                    app.debug_dump.max_files.max(1)
                                ),
                                Ok(DumpWrite::Off) => {}
                                Err(err) => warn!(
                                    target: "pwmd::peer",
                                    "divergence debug dump failed node_id={} err={}",
                                    node_id,
                                    err
                                ),
                            }
                        } else {
                            warn!(
                                target: "pwmd::peer",
                                "divergence debug dump skipped node_id={} reason=local_block_unavailable height={} hash={}",
                                node_id,
                                div.local_h,
                                div.local_hash
                            );
                        }
                    }
                    return SyncRouteOutcome::Disconnect {
                        reason: PeerCloseReason::SyncTipDivergence,
                        detail: format!(
                            "sync_tip_divergence local_h={} local_hash={} peer_h={} peer_hash={} cooldown_ms={}",
                            div.local_h, div.local_hash, div.peer_h, div.peer_hash, cooldown_ms
                        ),
                    };
                }
                Ok(None) => {
                    let mut hs = handshake_write_traced(&app, "cluster_route_stub").await;
                    if let Some(st) = hs.sync_live.peers.get_mut(node_id) {
                        st.div_streak = 0;
                    }
                }
                Err(err) => warn!(
                    target: "pwmd::peer",
                    "peer sync tip handling failed node_id={} err={}",
                    node_id,
                    err
                ),
            }
        }
        PeerWireMsg::SyncHeadersReq {
            from_height, limit, ..
        } => {
            if let Err(err) =
                sync_live::on_hdr_req(app, cfg, stream, from_height, limit, hdr_cap, seq_no).await
            {
                warn!(
                    target: "pwmd::peer",
                    "peer sync headers req handling failed node_id={} err={}",
                    node_id,
                    err
                );
            }
        }
        PeerWireMsg::SyncHeadersBatch { headers, .. } => {
            match sync_live::on_hdr_batch(
                app, cfg, stream, node_id, headers, hdr_cap, blk_cap, seq_no,
            )
            .await
            {
                Ok(Some(div)) => {
                    let mut hs = handshake_write_traced(&app, "cluster_route_stub").await;
                    hs.peer_scores.apply(node_id, PeerScoreEvent::ForkMismatch);
                    return SyncRouteOutcome::Disconnect {
                        reason: PeerCloseReason::SyncTipDivergence,
                        detail: format!(
                            "sync_hdr_divergence local_h={} local_hash={} peer_h={} peer_prev_hash={} cooldown_ms={}",
                            div.local_h,
                            div.local_hash,
                            div.peer_h,
                            div.peer_hash,
                            cfg.reconnect_runaway_cooldown_ms
                        ),
                    };
                }
                Ok(None) => {}
                Err(err) => {
                    warn!(
                        target: "pwmd::peer",
                        "peer sync headers batch handling failed node_id={} err={}",
                        node_id,
                        err
                    );
                }
            }
        }
        PeerWireMsg::SyncBlocksReq {
            block_hashes,
            block_heights,
            ..
        } => {
            if let Err(err) = sync_live::on_blk_req(
                app,
                cfg,
                stream,
                block_hashes,
                block_heights,
                blk_cap,
                seq_no,
            )
            .await
            {
                warn!(
                    target: "pwmd::peer",
                    "peer sync blocks req handling failed node_id={} err={}",
                    node_id,
                    err
                );
            }
        }
        PeerWireMsg::SyncBlocksBatch { blocks, .. } => {
            if let Err(err) =
                sync_live::on_blk_batch(app, cfg, stream, node_id, blocks, hdr_cap, blk_cap, seq_no)
                    .await
            {
                warn!(
                    target: "pwmd::peer",
                    "peer sync blocks batch handling failed node_id={} err={}",
                    node_id,
                    err
                );
            } else {
                let mut hs = handshake_write_traced(&app, "cluster_route_stub").await;
                hs.peer_scores.apply(node_id, PeerScoreEvent::ValidBlocks);
            }
        }
        PeerWireMsg::SyncNack { reason_code, .. } => {
            sync_live::on_nack(app, cfg, node_id, &reason_code).await;
        }
        PeerWireMsg::SyncCatchupReq {
            start_height,
            end_height,
            epoch_id,
            ..
        } => {
            if !can_cup {
                let mut hs = handshake_write_traced(&app, "cluster_route_stub").await;
                hs.transport.snapshot.sync_cup_drop_total =
                    hs.transport.snapshot.sync_cup_drop_total.saturating_add(1);
                return SyncRouteOutcome::Continue;
            }
            if let Err(err) =
                sync_live::on_cup_req(app, cfg, stream, start_height, end_height, epoch_id, seq_no)
                    .await
            {
                warn!(
                    target: "pwmd::peer",
                    "peer sync catchup req handling failed node_id={} err={}",
                    node_id,
                    err
                );
            }
        }
        PeerWireMsg::SyncCatchupChunk { chunk, .. } => {
            if !can_cup {
                let mut hs = handshake_write_traced(&app, "cluster_route_stub").await;
                hs.transport.snapshot.sync_cup_drop_total =
                    hs.transport.snapshot.sync_cup_drop_total.saturating_add(1);
                return SyncRouteOutcome::Continue;
            }
            sync_live::on_cup_chunk(app, cfg, node_id, chunk).await;
        }
        PeerWireMsg::SyncCatchupDone {
            epoch_id,
            last_height,
            last_hash,
            ..
        } => {
            if !can_cup {
                let mut hs = handshake_write_traced(&app, "cluster_route_stub").await;
                hs.transport.snapshot.sync_cup_drop_total =
                    hs.transport.snapshot.sync_cup_drop_total.saturating_add(1);
                return SyncRouteOutcome::Continue;
            }
            if let Err(err) = sync_live::on_cup_done(
                app,
                cfg,
                stream,
                node_id,
                epoch_id,
                last_height,
                &last_hash,
                hdr_cap,
                seq_no,
            )
            .await
            {
                warn!(
                    target: "pwmd::peer",
                    "peer sync catchup done handling failed node_id={} err={}",
                    node_id,
                    err
                );
            }
        }
        _ => {
            info!(
                target: "pwmd::peer",
                "peer sync frame accepted node_id={} session_id={} seq_no={} ts_ms={}",
                node_id,
                hdr.peer_session_id,
                hdr.seq_no,
                hdr.timestamp_ms
            );
        }
    }
    SyncRouteOutcome::Continue
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bootstrap::app_from_dev_net;
    use ed25519_dalek::SigningKey;
    use pwm_core::block::{hdr_hash, Block};
    use pwm_core::hd::{account_id_from_parts, domain_of_account_id};
    use pwm_core::tx::TxBody;
    use std::net::SocketAddr;

    fn sync_hdr_test(shard_id: u8) -> SyncWireHdr {
        SyncWireHdr {
            shard_id,
            peer_session_id: "peer-session".to_string(),
            seq_no: 1,
            timestamp_ms: current_time_ms().unwrap_or(0),
        }
    }

    async fn sync_tx_test(app: &App) -> SignedTx {
        let _ = app;
        let sk = SigningKey::from_bytes(&[11u8; 32]);
        let acct = account_id_from_parts(&sk.verifying_key().to_bytes(), 0);
        let dom = domain_of_account_id(&acct);
        SignedTx::sign_body(&sk, dom, 0, 0, TxBody::Init { index: 0, flags: 0 })
    }

    async fn remote_hello(app: &App, node_id: &str) -> NodeHello {
        let now_ms = current_time_ms().unwrap_or(0);
        let genesis_hash = {
            let hs = handshake_read_traced(&app, "peer_session_mod").await;
            hs.validation_ctx.expected_genesis_hash.clone()
        };
        let chain_tip_height = {
            let g = app.inner.read().await;
            Some(g.chain.tip_h())
        };
        let mut hello = build_local_node_hello(app, genesis_hash, None, now_ms, chain_tip_height);
        hello.node.node_id = node_id.to_string();
        hello
    }

    fn attest_sig_hex(
        sk: &SigningKey,
        h: u64,
        r: u32,
        vote: &str,
        cand: &str,
        candidate_ref: Option<&str>,
    ) -> String {
        let msg = cluster_sig_msg(h, r, vote, cand, candidate_ref);
        hex::encode(pwm_core::crypto::sign(sk, &msg))
    }

    async fn test_stream() -> (tokio::net::TcpStream, tokio::net::TcpStream) {
        let listener = tokio::net::TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0)))
            .await
            .expect("bind test listener");
        let addr = listener.local_addr().expect("listener local addr");
        let client_fut = tokio::spawn(async move { tokio::net::TcpStream::connect(addr).await });
        let (server, _) = listener.accept().await.expect("accept test peer");
        let client = client_fut
            .await
            .expect("join connect task")
            .expect("connect test peer");
        (server, client)
    }

    async fn route_test(
        app: &App,
        node_id: &str,
        msg: PeerWireMsg,
        full_v1: bool,
        same_shard: bool,
    ) {
        let (mut stream, _peer) = test_stream().await;
        let cfg = TransportConfig::default();
        let mut seq = 0u64;
        route_sync_stub(
            app,
            &cfg,
            &mut stream,
            Some("seed-route-test"),
            node_id,
            msg,
            full_v1,
            app.identity.cluster_domain_hi,
            same_shard,
            64,
            32,
            true,
            &mut seq,
        )
        .await;
    }

    async fn mk_remote_blk(app: &App) -> Block {
        let mut inn = app.inner.write().await;
        inn.chain.seal(Vec::new()).expect("seal test block");
        let blk = inn
            .chain
            .blocks
            .back()
            .cloned()
            .expect("sealed block must exist");
        inn.chain.blocks.clear();
        inn.chain.st = inn.chain.cfg.state0();
        inn.chain.set_canon_h(0);
        blk
    }

    async fn mk_remote_blks(app: &App, count: usize) -> Vec<Block> {
        let mut inn = app.inner.write().await;
        for _ in 0..count {
            inn.chain.seal(Vec::new()).expect("seal test block");
        }
        let out = inn.chain.blocks.iter().cloned().collect::<Vec<_>>();
        inn.chain.blocks.clear();
        inn.chain.st = inn.chain.cfg.state0();
        inn.chain.set_canon_h(0);
        out
    }

    /// Build one CUP chunk wire (`≤32` blocks) for epoch `0` tests.
    fn cup_chunk_wire(slice: &[Block], chunk_index: u32) -> SyncCatchupChunkWire {
        let first = slice.first().expect("chunk non-empty");
        let last = slice.last().expect("chunk non-empty");
        SyncCatchupChunkWire {
            epoch_id: 0,
            chunk_index,
            first_prev_hash: hex::encode(first.hdr.prev_hash),
            last_hash: hex::encode(hdr_hash(&last.hdr)),
            headers: slice
                .iter()
                .map(|b| SyncHeaderWire {
                    height: b.hdr.height,
                    hash: hex::encode(hdr_hash(&b.hdr)),
                    prev_hash: hex::encode(b.hdr.prev_hash),
                })
                .collect(),
            blocks: slice
                .iter()
                .map(|b| SyncBlockWire {
                    height: b.hdr.height,
                    hash: hex::encode(hdr_hash(&b.hdr)),
                    block: Some(b.clone()),
                })
                .collect(),
        }
    }

    #[tokio::test]
    async fn tx_batch_valid_in() {
        let app = app_from_dev_net();
        let tx = sync_tx_test(&app).await;
        let msg = PeerWireMsg::SyncTxBatch {
            hdr: sync_hdr_test(app.identity.cluster_domain_hi),
            txs: vec![tx],
        };
        route_test(&app, "peer-valid", msg, true, true).await;
        {
            let hs = handshake_read_traced(&app, "peer_session_mod").await;
            assert_eq!(hs.transport.snapshot.sync_tx_seen_total, 1);
            assert_eq!(hs.transport.snapshot.sync_tx_accept_total, 1);
            assert_eq!(hs.transport.snapshot.sync_tx_drop_total, 0);
        }
        let g = app.inner.read().await;
        assert_eq!(g.pool.snapshot(8).len(), 1);
    }

    #[tokio::test]
    async fn tx_batch_dup_drop() {
        let app = app_from_dev_net();
        let tx = sync_tx_test(&app).await;
        let mk_msg = || PeerWireMsg::SyncTxBatch {
            hdr: sync_hdr_test(app.identity.cluster_domain_hi),
            txs: vec![tx.clone()],
        };
        route_test(&app, "peer-dup", mk_msg(), true, true).await;
        route_test(&app, "peer-dup", mk_msg(), true, true).await;
        let hs = handshake_read_traced(&app, "peer_session_mod").await;
        assert_eq!(hs.transport.snapshot.sync_tx_accept_total, 1);
        assert_eq!(hs.transport.snapshot.sync_tx_drop_total, 1);
        assert_eq!(
            hs.transport
                .snapshot
                .sync_tx_drop_reason
                .get("duplicate")
                .copied(),
            Some(1)
        );
    }

    #[tokio::test]
    async fn tx_batch_profile_drop() {
        let app = app_from_dev_net();
        let tx = sync_tx_test(&app).await;
        let msg = PeerWireMsg::SyncTxBatch {
            hdr: sync_hdr_test(app.identity.cluster_domain_hi),
            txs: vec![tx],
        };
        route_test(&app, "peer-legacy", msg, false, true).await;
        {
            let hs = handshake_read_traced(&app, "peer_session_mod").await;
            assert_eq!(hs.transport.snapshot.sync_tx_accept_total, 0);
            assert_eq!(hs.transport.snapshot.sync_tx_drop_total, 1);
            assert_eq!(
                hs.transport
                    .snapshot
                    .sync_tx_drop_reason
                    .get("same_shard_profile_mismatch")
                    .copied(),
                Some(1)
            );
        }
        let g = app.inner.read().await;
        assert!(g.pool.snapshot(8).is_empty());
    }

    #[tokio::test]
    async fn tx_batch_inter_shard_drop() {
        let app = app_from_dev_net();
        let tx = sync_tx_test(&app).await;
        let msg = PeerWireMsg::SyncTxBatch {
            hdr: sync_hdr_test(app.identity.cluster_domain_hi),
            txs: vec![tx],
        };
        route_test(&app, "peer-foreign", msg, true, false).await;
        {
            let hs = handshake_read_traced(&app, "peer_session_mod").await;
            assert_eq!(hs.transport.snapshot.sync_tx_accept_total, 0);
            assert_eq!(hs.transport.snapshot.sync_tx_drop_total, 1);
            assert_eq!(
                hs.transport
                    .snapshot
                    .sync_tx_drop_reason
                    .get("inter_shard_sync_forbidden")
                    .copied(),
                Some(1)
            );
            assert_eq!(
                hs.transport
                    .snapshot
                    .sync_v1_drop_reason
                    .get("inter_shard_sync_forbidden")
                    .copied(),
                Some(1)
            );
        }
        let g = app.inner.read().await;
        assert!(g.pool.snapshot(8).is_empty());
    }

    #[tokio::test]
    async fn legacy_sync_hdr_safe() {
        let app = app_from_dev_net();
        let msg = PeerWireMsg::SyncHeadersReq {
            hdr: sync_hdr_test(app.identity.cluster_domain_hi),
            from_height: 1,
            limit: 16,
        };
        route_test(&app, "peer-legacy-hdr", msg, false, true).await;
        let hs = handshake_read_traced(&app, "peer_session_mod").await;
        assert_eq!(hs.transport.snapshot.sync_v1_seen_total, 1);
        assert_eq!(hs.transport.snapshot.sync_v1_drop_total, 1);
        assert_eq!(hs.transport.snapshot.sync_tx_seen_total, 0);
        assert_eq!(hs.transport.snapshot.sync_tx_accept_total, 0);
        assert_eq!(hs.transport.snapshot.sync_tx_drop_total, 0);
    }

    #[tokio::test]
    async fn hdr_batch_break_drop() {
        let app = app_from_dev_net();
        let (mut stream, _peer) = test_stream().await;
        let cfg = TransportConfig::default();
        let mut seq = 0u64;
        sync_live::on_tip(
            &app,
            &cfg,
            &mut stream,
            "peer-a",
            1,
            "aa",
            0,
            None,
            64,
            true,
            &mut seq,
        )
        .await
        .expect("tip route");
        let bad = vec![SyncHeaderWire {
            height: 1,
            hash: "11".repeat(32),
            prev_hash: "22".repeat(32),
        }];
        let out = sync_live::on_hdr_batch(&app, &cfg, &mut stream, "peer-a", bad, 64, 32, &mut seq)
            .await
            .expect("hdr batch route");
        assert!(out.is_none());
        let hs = handshake_read_traced(&app, "peer_session_mod").await;
        assert_eq!(hs.transport.snapshot.sync_fork_conflict_total, 1);
    }

    #[tokio::test]
    async fn blk_fetch_apply_ok() {
        let app = app_from_dev_net();
        assert!(
            !app.cluster_cfg.enabled,
            "follower sync path must not require cluster quorum"
        );
        {
            let mut hs = handshake_write_traced(&app, "peer_session_mod").await;
            hs.trusted_peers.insert(
                "peer-b".to_string(),
                crate::transport::TrustedPeer {
                    node_id: "peer-b".to_string(),
                    cluster_id: app.identity.cluster_id.clone(),
                    pubkey: SigningKey::from_bytes(&[1u8; 32])
                        .verifying_key()
                        .to_bytes(),
                    domain_hi: app.identity.cluster_domain_hi,
                    instance_id: Some("peer-b".to_string()),
                    cluster_attest_enabled: false,
                    cluster_role: crate::handshake::ClusterRole::None,
                },
            );
        }
        let blk = mk_remote_blk(&app).await;
        let blk_hash = hex::encode(hdr_hash(&blk.hdr));
        let (mut stream, _peer) = test_stream().await;
        let cfg = TransportConfig::default();
        let mut seq = 0u64;
        sync_live::on_tip(
            &app,
            &cfg,
            &mut stream,
            "peer-b",
            1,
            &blk_hash,
            0,
            None,
            64,
            true,
            &mut seq,
        )
        .await
        .expect("tip route");
        let hdrs = vec![SyncHeaderWire {
            height: blk.hdr.height,
            hash: blk_hash.clone(),
            prev_hash: hex::encode(blk.hdr.prev_hash),
        }];
        let out =
            sync_live::on_hdr_batch(&app, &cfg, &mut stream, "peer-b", hdrs, 64, 32, &mut seq)
                .await
                .expect("hdr batch route");
        assert!(out.is_none());
        let rows = vec![SyncBlockWire {
            height: blk.hdr.height,
            hash: blk_hash.clone(),
            block: Some(blk),
        }];
        sync_live::on_blk_batch(&app, &cfg, &mut stream, "peer-b", rows, 64, 32, &mut seq)
            .await
            .expect("blk batch route");
        let inn = app.inner.read().await;
        assert_eq!(inn.chain.tip_h(), 0);
        assert_ne!(hex::encode(inn.chain.tip_hash()), blk_hash);
    }

    #[tokio::test]
    async fn blk_bad_reject_safe() {
        let app = app_from_dev_net();
        let mut blk = mk_remote_blk(&app).await;
        blk.hdr.state_root = [9u8; 32];
        let blk_hash = hex::encode(hdr_hash(&blk.hdr));
        let (mut stream, _peer) = test_stream().await;
        let cfg = TransportConfig::default();
        let mut seq = 0u64;
        sync_live::on_tip(
            &app,
            &cfg,
            &mut stream,
            "peer-c",
            1,
            &blk_hash,
            0,
            None,
            64,
            true,
            &mut seq,
        )
        .await
        .expect("tip route");
        let hdrs = vec![SyncHeaderWire {
            height: blk.hdr.height,
            hash: blk_hash.clone(),
            prev_hash: hex::encode(blk.hdr.prev_hash),
        }];
        let out =
            sync_live::on_hdr_batch(&app, &cfg, &mut stream, "peer-c", hdrs, 64, 32, &mut seq)
                .await
                .expect("hdr batch route");
        assert!(out.is_none());
        let rows = vec![SyncBlockWire {
            height: blk.hdr.height,
            hash: blk_hash,
            block: Some(blk),
        }];
        sync_live::on_blk_batch(&app, &cfg, &mut stream, "peer-c", rows, 64, 32, &mut seq)
            .await
            .expect("blk batch route");
        let hs = handshake_read_traced(&app, "peer_session_mod").await;
        assert_eq!(hs.transport.snapshot.sync_apply_fail_total, 1);
        let inn = app.inner.read().await;
        assert_eq!(inn.chain.tip_h(), 0);
    }

    #[tokio::test]
    async fn cup_missing_range_ok() {
        const DEEP_LAG: usize = 256;
        let app = app_from_dev_net();
        {
            let mut hs = handshake_write_traced(&app, "peer_session_mod").await;
            hs.trusted_peers.insert(
                "peer-cup-ok".to_string(),
                crate::transport::TrustedPeer {
                    node_id: "peer-cup-ok".to_string(),
                    cluster_id: app.identity.cluster_id.clone(),
                    pubkey: SigningKey::from_bytes(&[2u8; 32])
                        .verifying_key()
                        .to_bytes(),
                    domain_hi: app.identity.cluster_domain_hi,
                    instance_id: Some("peer-cup-ok".to_string()),
                    cluster_attest_enabled: false,
                    cluster_role: crate::handshake::ClusterRole::None,
                },
            );
        }
        let blks = mk_remote_blks(&app, DEEP_LAG).await;
        let (mut stream, _peer) = test_stream().await;
        let cfg = TransportConfig::default();
        let mut seq = 0u64;
        sync_live::on_nack(&app, &cfg, "peer-cup-ok", "stall").await;
        sync_live::on_nack(&app, &cfg, "peer-cup-ok", "stall").await;
        let last = blks.last().expect("remote tip");
        let last_hash = hex::encode(hdr_hash(&last.hdr));
        sync_live::on_tip(
            &app,
            &cfg,
            &mut stream,
            "peer-cup-ok",
            last.hdr.height,
            &last_hash,
            0,
            None,
            64,
            true,
            &mut seq,
        )
        .await
        .expect("tip route");
        for chunk_i in 0..(DEEP_LAG / 32) {
            let start = chunk_i * 32;
            let chunk_blks = &blks[start..start + 32];
            let chunk = cup_chunk_wire(chunk_blks, chunk_i as u32);
            sync_live::on_cup_chunk(&app, &cfg, "peer-cup-ok", chunk).await;
        }
        sync_live::on_cup_done(
            &app,
            &cfg,
            &mut stream,
            "peer-cup-ok",
            0,
            last.hdr.height,
            &last_hash,
            64,
            &mut seq,
        )
        .await
        .expect("cup done");
        let inn = app.inner.read().await;
        assert_eq!(inn.chain.tip_h(), 0);
    }

    #[tokio::test]
    async fn cup_bad_chunk_safe() {
        const DEEP_LAG: usize = 256;
        let app = app_from_dev_net();
        let blks = mk_remote_blks(&app, DEEP_LAG).await;
        let (mut stream, _peer) = test_stream().await;
        let cfg = TransportConfig::default();
        let mut seq = 0u64;
        sync_live::on_nack(&app, &cfg, "peer-cup-bad", "stall").await;
        sync_live::on_nack(&app, &cfg, "peer-cup-bad", "stall").await;
        let last = blks.last().expect("remote tip");
        let last_hash = hex::encode(hdr_hash(&last.hdr));
        sync_live::on_tip(
            &app,
            &cfg,
            &mut stream,
            "peer-cup-bad",
            last.hdr.height,
            &last_hash,
            0,
            None,
            64,
            true,
            &mut seq,
        )
        .await
        .expect("tip route");
        let chunk0 = &blks[0..32];
        let bad_chunk = SyncCatchupChunkWire {
            epoch_id: 0,
            chunk_index: 0,
            first_prev_hash: hex::encode(chunk0[0].hdr.prev_hash),
            last_hash: "11".repeat(32),
            headers: chunk0
                .iter()
                .map(|b| SyncHeaderWire {
                    height: b.hdr.height,
                    hash: hex::encode(hdr_hash(&b.hdr)),
                    prev_hash: hex::encode(b.hdr.prev_hash),
                })
                .collect(),
            blocks: chunk0
                .iter()
                .map(|b| SyncBlockWire {
                    height: b.hdr.height,
                    hash: "22".repeat(32),
                    block: Some(b.clone()),
                })
                .collect(),
        };
        sync_live::on_cup_chunk(&app, &cfg, "peer-cup-bad", bad_chunk).await;
        let hs = handshake_read_traced(&app, "peer_session_mod").await;
        assert!(hs.transport.snapshot.sync_cup_fail_total >= 1);
        let inn = app.inner.read().await;
        assert_eq!(inn.chain.tip_h(), 0);
    }

    #[tokio::test]
    async fn cup_nack_resets_state() {
        const DEEP_LAG: usize = 256;
        let app = app_from_dev_net();
        let blks = mk_remote_blks(&app, DEEP_LAG).await;
        let (mut stream, _peer) = test_stream().await;
        let cfg = TransportConfig::default();
        let mut seq = 0u64;
        sync_live::on_nack(&app, &cfg, "peer-cup-nack", "stall").await;
        sync_live::on_nack(&app, &cfg, "peer-cup-nack", "stall").await;
        let last = blks.last().expect("remote tip");
        let last_hash = hex::encode(hdr_hash(&last.hdr));
        sync_live::on_tip(
            &app,
            &cfg,
            &mut stream,
            "peer-cup-nack",
            last.hdr.height,
            &last_hash,
            0,
            None,
            64,
            true,
            &mut seq,
        )
        .await
        .expect("tip starts catchup");
        {
            let hs = handshake_read_traced(&app, "peer_session_mod").await;
            let st = hs.sync_live.peers.get("peer-cup-nack").expect("peer state");
            assert!(st.cup_active);
        }
        sync_live::on_nack(&app, &cfg, "peer-cup-nack", "catchup_range").await;
        {
            let hs = handshake_read_traced(&app, "peer_session_mod").await;
            let st = hs.sync_live.peers.get("peer-cup-nack").expect("peer state");
            assert!(!st.cup_active);
            assert_eq!(st.cup_try, 1);
            assert!(st.cup_next_ms > 0);
        }
        sync_live::on_tip(
            &app,
            &cfg,
            &mut stream,
            "peer-cup-nack",
            last.hdr.height,
            &last_hash,
            0,
            None,
            64,
            true,
            &mut seq,
        )
        .await
        .expect("tip fallback live");
        let hs = handshake_read_traced(&app, "peer_session_mod").await;
        let st = hs.sync_live.peers.get("peer-cup-nack").expect("peer state");
        assert_eq!(st.wait_hdr_from, Some(1));
        assert_eq!(hs.transport.snapshot.sync_cup_fail_total, 1);
    }

    #[tokio::test]
    async fn cup_send_fail_resets() {
        const DEEP_LAG: usize = 256;
        let app = app_from_dev_net();
        let blks = mk_remote_blks(&app, DEEP_LAG).await;
        let (mut stream, _peer) = test_stream().await;
        let cfg = TransportConfig::default();
        let mut seq = 0u64;
        use tokio::io::AsyncWriteExt;
        stream.shutdown().await.expect("shutdown write half");
        sync_live::on_nack(&app, &cfg, "peer-cup-fail", "stall").await;
        sync_live::on_nack(&app, &cfg, "peer-cup-fail", "stall").await;
        let last = blks.last().expect("remote tip");
        let last_hash = hex::encode(hdr_hash(&last.hdr));
        let _ = sync_live::on_tip(
            &app,
            &cfg,
            &mut stream,
            "peer-cup-fail",
            last.hdr.height,
            &last_hash,
            0,
            None,
            64,
            true,
            &mut seq,
        )
        .await;
        let hs = handshake_read_traced(&app, "peer_session_mod").await;
        let st = hs.sync_live.peers.get("peer-cup-fail").expect("peer state");
        assert!(!st.cup_active);
        assert_eq!(st.cup_try, 1);
        assert_eq!(hs.transport.snapshot.sync_cup_fail_total, 1);
        assert_eq!(
            hs.transport
                .snapshot
                .sync_cup_fail_reason
                .get("req_write")
                .copied()
                .unwrap_or(0),
            1
        );
    }

    #[tokio::test]
    async fn cup_profile_mismatch_noop() {
        let app = app_from_dev_net();
        let (mut stream, _peer) = test_stream().await;
        let cfg = TransportConfig::default();
        let mut seq = 0u64;
        route_sync_stub(
            &app,
            &cfg,
            &mut stream,
            Some("seed-cup-mismatch"),
            "peer-cup-mismatch",
            PeerWireMsg::SyncCatchupReq {
                hdr: sync_hdr_test(app.identity.cluster_domain_hi),
                start_height: 1,
                end_height: 2,
                epoch_id: 0,
                anchor_hash: "00".repeat(32),
            },
            true,
            app.identity.cluster_domain_hi,
            true,
            64,
            32,
            false,
            &mut seq,
        )
        .await;
        let hs = handshake_read_traced(&app, "peer_session_mod").await;
        assert_eq!(hs.transport.snapshot.sync_cup_drop_total, 1);
        let inn = app.inner.read().await;
        assert_eq!(inn.chain.tip_h(), 0);
    }

    #[tokio::test]
    async fn sync_shard_drop_noop() {
        let app = app_from_dev_net();
        let msg = PeerWireMsg::SyncTipAnnounce {
            hdr: sync_hdr_test(app.identity.cluster_domain_hi.saturating_add(1)),
            head_height: 5,
            head_hash: "33".repeat(32),
            finalized_height: 5,
            finalized_hash: None,
        };
        route_test(&app, "peer-shard", msg, true, true).await;
        let hs = handshake_read_traced(&app, "peer_session_mod").await;
        assert_eq!(hs.transport.snapshot.sync_v1_drop_total, 1);
        assert_eq!(
            hs.transport
                .snapshot
                .sync_v1_drop_reason
                .get("shard_mismatch")
                .copied(),
            Some(1)
        );
        assert_eq!(hs.transport.snapshot.sync_hdr_req_total, 0);
    }

    #[tokio::test]
    async fn tip_divergence_disconnect_marks_backoff() {
        let app = app_from_dev_net();
        let (mut stream, _peer) = test_stream().await;
        let cfg = TransportConfig::default();
        let mut seq = 0u64;
        let (local_h, local_hash) = {
            let g = app.inner.read().await;
            (g.chain.tip_h(), hex::encode(g.chain.tip_hash()))
        };
        let peer_hash = if local_hash.starts_with('0') {
            format!("1{}", &local_hash[1..])
        } else {
            format!("0{}", &local_hash[1..])
        };
        let now_ms = current_time_ms().unwrap_or(0);
        let out = route_sync_stub(
            &app,
            &cfg,
            &mut stream,
            Some("seed-divergence"),
            "peer-divergence",
            PeerWireMsg::SyncTipAnnounce {
                hdr: sync_hdr_test(app.identity.cluster_domain_hi),
                head_height: local_h,
                head_hash: peer_hash,
                finalized_height: local_h,
                finalized_hash: None,
            },
            true,
            app.identity.cluster_domain_hi,
            true,
            64,
            32,
            true,
            &mut seq,
        )
        .await;
        assert!(matches!(
            out,
            SyncRouteOutcome::Disconnect {
                reason: PeerCloseReason::SyncTipDivergence,
                ..
            }
        ));
        let hs = handshake_read_traced(&app, "peer_session_mod").await;
        assert_eq!(hs.transport.snapshot.sync_tip_disconnect_total, 1);
        let due_ms = hs
            .transport
            .seed_peers
            .get("seed-divergence")
            .map(|x| x.next_due_ms)
            .unwrap_or(0);
        assert!(due_ms.saturating_sub(now_ms) >= 59_000);
    }

    #[tokio::test]
    async fn tip_divergence_height_skip() {
        let app = app_from_dev_net();
        let (mut stream, _peer) = test_stream().await;
        let cfg = TransportConfig::default();
        let mut seq = 0u64;
        let local_h = {
            let g = app.inner.read().await;
            g.chain.tip_h()
        };
        let out = route_sync_stub(
            &app,
            &cfg,
            &mut stream,
            Some("seed-divergence-skip"),
            "peer-divergence-skip",
            PeerWireMsg::SyncTipAnnounce {
                hdr: sync_hdr_test(app.identity.cluster_domain_hi),
                head_height: local_h.saturating_add(1),
                head_hash: "44".repeat(32),
                finalized_height: local_h.saturating_add(1),
                finalized_hash: None,
            },
            true,
            app.identity.cluster_domain_hi,
            true,
            64,
            32,
            true,
            &mut seq,
        )
        .await;
        assert_eq!(out, SyncRouteOutcome::Continue);
        let hs = handshake_read_traced(&app, "peer_session_mod").await;
        assert_eq!(hs.transport.snapshot.sync_tip_disconnect_total, 0);
    }

    #[tokio::test]
    async fn tip_behind_no_divergence() {
        let app = app_from_dev_net();
        let genesis_hash = {
            let g = app.inner.read().await;
            assert_eq!(g.chain.tip_h(), 0);
            hex::encode(g.chain.tip_hash())
        };
        {
            let mut inn = app.inner.write().await;
            inn.chain.seal(Vec::new()).expect("seal local block");
            assert_eq!(inn.chain.tip_h(), 1);
        }
        let (mut stream, _peer) = test_stream().await;
        let cfg = TransportConfig::default();
        let mut seq = 0u64;
        let out = route_sync_stub(
            &app,
            &cfg,
            &mut stream,
            Some("seed-behind"),
            "peer-behind",
            PeerWireMsg::SyncTipAnnounce {
                hdr: sync_hdr_test(app.identity.cluster_domain_hi),
                head_height: 0,
                head_hash: genesis_hash,
                finalized_height: 0,
                finalized_hash: None,
            },
            true,
            app.identity.cluster_domain_hi,
            true,
            64,
            32,
            true,
            &mut seq,
        )
        .await;
        assert_eq!(out, SyncRouteOutcome::Continue);
        let hs = handshake_read_traced(&app, "peer_session_mod").await;
        assert_eq!(hs.transport.snapshot.sync_tip_disconnect_total, 0);
    }

    #[tokio::test]
    async fn tip_divergence_inbound_seed_cooldown() {
        let app = app_from_dev_net();
        let (mut stream, _peer) = test_stream().await;
        let cfg = TransportConfig::default();
        let mut seq = 0u64;
        {
            let mut hs = handshake_write_traced(&app, "cluster_route_stub").await;
            hs.transport.seed_peers.insert(
                "seed-inbound-divergence".to_string(),
                TransportPeerState {
                    last_node_id: Some("peer-inbound-divergence".to_string()),
                    ..Default::default()
                },
            );
        }
        let (local_h, local_hash) = {
            let g = app.inner.read().await;
            (g.chain.tip_h(), hex::encode(g.chain.tip_hash()))
        };
        let peer_hash = if local_hash.starts_with('0') {
            format!("1{}", &local_hash[1..])
        } else {
            format!("0{}", &local_hash[1..])
        };
        let now_ms = current_time_ms().unwrap_or(0);
        let out = route_sync_stub(
            &app,
            &cfg,
            &mut stream,
            None,
            "peer-inbound-divergence",
            PeerWireMsg::SyncTipAnnounce {
                hdr: sync_hdr_test(app.identity.cluster_domain_hi),
                head_height: local_h,
                head_hash: peer_hash,
                finalized_height: local_h,
                finalized_hash: None,
            },
            true,
            app.identity.cluster_domain_hi,
            true,
            64,
            32,
            true,
            &mut seq,
        )
        .await;
        assert!(matches!(
            out,
            SyncRouteOutcome::Disconnect {
                reason: PeerCloseReason::SyncTipDivergence,
                ..
            }
        ));
        let hs = handshake_read_traced(&app, "peer_session_mod").await;
        let due_ms = hs
            .transport
            .seed_peers
            .get("seed-inbound-divergence")
            .map(|x| x.next_due_ms)
            .unwrap_or(0);
        assert!(due_ms.saturating_sub(now_ms) >= 59_000);
    }

    #[tokio::test]
    async fn tip_divergence_prefers_settled_anchor() {
        let app = app_from_dev_net();
        {
            let mut inn = app.inner.write().await;
            inn.chain.seal(Vec::new()).expect("seal test block");
            inn.chain.seal(Vec::new()).expect("seal test block");
        }
        let (mut stream, _peer) = test_stream().await;
        let cfg = TransportConfig::default();
        let mut seq = 0u64;
        let (local_h, local_hash, anchor_h, anchor_hash) = {
            let g = app.inner.read().await;
            let local_h = g.chain.tip_h();
            let local_hash = hex::encode(g.chain.tip_hash());
            let anchor_h = local_h.saturating_sub(1);
            let anchor_hash = g
                .chain
                .blocks
                .iter()
                .rev()
                .find(|b| b.hdr.height == anchor_h)
                .map(|b| hex::encode(hdr_hash(&b.hdr)))
                .expect("anchor hash");
            (local_h, local_hash, anchor_h, anchor_hash)
        };
        let peer_head_hash = if local_hash.starts_with('0') {
            format!("1{}", &local_hash[1..])
        } else {
            format!("0{}", &local_hash[1..])
        };
        let out = route_sync_stub(
            &app,
            &cfg,
            &mut stream,
            Some("seed-divergence-anchor"),
            "peer-divergence-anchor",
            PeerWireMsg::SyncTipAnnounce {
                hdr: sync_hdr_test(app.identity.cluster_domain_hi),
                head_height: local_h,
                head_hash: peer_head_hash,
                finalized_height: anchor_h,
                finalized_hash: Some(anchor_hash),
            },
            true,
            app.identity.cluster_domain_hi,
            true,
            64,
            32,
            true,
            &mut seq,
        )
        .await;
        assert_eq!(out, SyncRouteOutcome::Continue);
        let hs = handshake_read_traced(&app, "peer_session_mod").await;
        assert_eq!(hs.transport.snapshot.sync_tip_disconnect_total, 0);
    }

    #[tokio::test]
    async fn live_reconnect_sync_no_deadlock() {
        let app = app_from_dev_net();
        let (mut stream, _peer) = test_stream().await;
        let cfg = TransportConfig::default();
        let mut seq = 0u64;
        sync_live::on_tip(
            &app,
            &cfg,
            &mut stream,
            "peer-reconnect",
            2,
            "aa",
            0,
            None,
            64,
            false,
            &mut seq,
        )
        .await
        .expect("first tip");
        {
            let hs = handshake_read_traced(&app, "peer_session_mod").await;
            let st = hs
                .sync_live
                .peers
                .get("peer-reconnect")
                .expect("peer state");
            assert_eq!(st.in_hdr, 1);
            assert_eq!(st.wait_hdr_from, Some(1));
        }
        sync_live::on_nack(&app, &cfg, "peer-reconnect", "wire_read_failed").await;
        {
            let hs = handshake_read_traced(&app, "peer_session_mod").await;
            let st = hs
                .sync_live
                .peers
                .get("peer-reconnect")
                .expect("peer state");
            assert_eq!(st.in_hdr, 0);
            assert_eq!(st.wait_hdr_from, None);
        }
        sync_live::on_tip(
            &app,
            &cfg,
            &mut stream,
            "peer-reconnect",
            2,
            "aa",
            0,
            None,
            64,
            false,
            &mut seq,
        )
        .await
        .expect("second tip");
        let hs = handshake_read_traced(&app, "peer_session_mod").await;
        let st = hs
            .sync_live
            .peers
            .get("peer-reconnect")
            .expect("peer state");
        assert_eq!(st.in_hdr, 1);
        assert_eq!(st.wait_hdr_from, Some(1));
        assert!(hs.transport.snapshot.sync_hdr_req_total >= 2);
    }

    #[tokio::test]
    async fn storm_egress_not_blackhole() {
        let app = app_from_dev_net();
        let tx = sync_tx_test(&app).await;
        {
            let mut g = app.inner.write().await;
            g.pool.push(tx.clone()).expect("push tx to pool");
        }
        let (mut stream_a, _peer_a) = test_stream().await;
        let (mut stream_b, _peer_b) = test_stream().await;
        let cfg = TransportConfig::default();
        let mut seq = 0u64;
        let remote_a = remote_hello(&app, "peer-egress-a").await;
        let remote_b = remote_hello(&app, "peer-egress-b").await;
        send_sync_tx_batch(&app, &cfg, &mut stream_a, &remote_a, &mut seq)
            .await
            .expect("first relay");
        send_sync_tx_batch(&app, &cfg, &mut stream_a, &remote_a, &mut seq)
            .await
            .expect("suppressed same peer");
        send_sync_tx_batch(&app, &cfg, &mut stream_b, &remote_b, &mut seq)
            .await
            .expect("relay second peer");
        let hs = handshake_read_traced(&app, "peer_session_mod").await;
        assert!(
            hs.transport
                .snapshot
                .mempool_push_suppressed
                .get("recent_peer_dedup")
                .copied()
                .unwrap_or(0)
                >= 1
        );
        assert!(
            hs.transport
                .snapshot
                .mempool_egress_relay_total
                .get("same_shard_peer")
                .copied()
                .unwrap_or(0)
                >= 2
        );
    }

    #[tokio::test]
    async fn cluster_attest_unsigned_drop() {
        let mut app = app_from_dev_net();
        app.cluster_cfg.enabled = true;
        app.cluster_cfg.role = crate::handshake::ClusterRole::Proposer;
        app.cluster_cfg.members = vec!["node-a".to_string(), "node-b".to_string()];
        app.cluster_cfg.quorum_n = 2;
        app.cluster_cfg.quorum_k = 1;
        let h = app.inner.read().await.chain.tip_h().saturating_add(1);
        {
            let mut hs = handshake_write_traced(&app, "cluster_route_stub").await;
            hs.trusted_peers.insert(
                "peer-b".to_string(),
                crate::transport::TrustedPeer {
                    node_id: "peer-b".to_string(),
                    cluster_id: "cluster-a".to_string(),
                    pubkey: SigningKey::from_bytes(&[9u8; 32])
                        .verifying_key()
                        .to_bytes(),
                    domain_hi: app.identity.cluster_domain_hi,
                    instance_id: Some("node-b".to_string()),
                    cluster_attest_enabled: true,
                    cluster_role: crate::handshake::ClusterRole::Attester,
                },
            );
            let round = hs.cluster_attest.rounds.entry((h, 0)).or_default();
            round.vote_object = "vo1".to_string();
            round.candidate_hash = "ab".repeat(32);
            round.proposer_id = Some("node-a".to_string());
            round.propose_opened_at_ms = Some(crate::current_time_ms().unwrap_or(0));
        }
        route_cluster_stub(
            &app,
            "peer-b",
            PeerWireMsg::ClusterAttest {
                msg: ClusterAttestWire {
                    height: h,
                    round: 0,
                    vote_object: "vo1".to_string(),
                    candidate_hash: "ab".repeat(32),
                    signature: "not-a-signature".to_string(),
                    candidate_ref: None,
                    attester_tip_height: None,
                },
            },
        )
        .await;
        let hs = handshake_read_traced(&app, "peer_session_mod").await;
        let got = hs
            .cluster_attest
            .rounds
            .get(&(h, 0))
            .map(|x| x.attesters.len())
            .unwrap_or(0);
        assert_eq!(got, 0);
    }

    #[tokio::test]
    async fn cluster_attest_non_member_drop() {
        let mut app = app_from_dev_net();
        app.cluster_cfg.enabled = true;
        app.cluster_cfg.role = crate::handshake::ClusterRole::Proposer;
        app.cluster_cfg.members = vec!["node-a".to_string(), "node-b".to_string()];
        app.cluster_cfg.quorum_n = 2;
        app.cluster_cfg.quorum_k = 1;
        let h = app.inner.read().await.chain.tip_h().saturating_add(1);
        let sk = SigningKey::from_bytes(&[8u8; 32]);
        {
            let mut hs = handshake_write_traced(&app, "cluster_route_stub").await;
            hs.trusted_peers.insert(
                "peer-c".to_string(),
                crate::transport::TrustedPeer {
                    node_id: "peer-c".to_string(),
                    cluster_id: "cluster-a".to_string(),
                    pubkey: sk.verifying_key().to_bytes(),
                    domain_hi: app.identity.cluster_domain_hi,
                    instance_id: Some("node-x".to_string()),
                    cluster_attest_enabled: true,
                    cluster_role: crate::handshake::ClusterRole::Attester,
                },
            );
            let round = hs.cluster_attest.rounds.entry((h, 0)).or_default();
            round.vote_object = "vo2".to_string();
            round.candidate_hash = "cd".repeat(32);
            round.proposer_id = Some("node-a".to_string());
            round.propose_opened_at_ms = Some(crate::current_time_ms().unwrap_or(0));
        }
        route_cluster_stub(
            &app,
            "peer-c",
            PeerWireMsg::ClusterAttest {
                msg: ClusterAttestWire {
                    height: h,
                    round: 0,
                    vote_object: "vo2".to_string(),
                    candidate_hash: "cd".repeat(32),
                    signature: attest_sig_hex(&sk, h, 0, "vo2", &"cd".repeat(32), None),
                    candidate_ref: None,
                    attester_tip_height: None,
                },
            },
        )
        .await;
        let hs = handshake_read_traced(&app, "peer_session_mod").await;
        let got = hs
            .cluster_attest
            .rounds
            .get(&(h, 0))
            .map(|x| x.attesters.len())
            .unwrap_or(0);
        assert_eq!(got, 0);
    }

    #[tokio::test]
    async fn cluster_attest_accepts_valid_signature() {
        let mut app = app_from_dev_net();
        app.cluster_cfg.enabled = true;
        app.cluster_cfg.role = crate::handshake::ClusterRole::Proposer;
        app.cluster_cfg.members = vec!["node-a".to_string(), "node-b".to_string()];
        app.cluster_cfg.quorum_n = 2;
        app.cluster_cfg.quorum_k = 1;
        let h = app.inner.read().await.chain.tip_h().saturating_add(1);
        let sk = SigningKey::from_bytes(&[7u8; 32]);
        {
            let mut hs = handshake_write_traced(&app, "cluster_route_stub").await;
            hs.trusted_peers.insert(
                "peer-b".to_string(),
                crate::transport::TrustedPeer {
                    node_id: "peer-b".to_string(),
                    cluster_id: "cluster-a".to_string(),
                    pubkey: sk.verifying_key().to_bytes(),
                    domain_hi: app.identity.cluster_domain_hi,
                    instance_id: Some("node-b".to_string()),
                    cluster_attest_enabled: true,
                    cluster_role: crate::handshake::ClusterRole::Attester,
                },
            );
            let round = hs.cluster_attest.rounds.entry((h, 0)).or_default();
            round.vote_object = "vo1".to_string();
            round.candidate_hash = "ef".repeat(32);
            round.proposer_id = Some("node-a".to_string());
            round.propose_opened_at_ms = Some(crate::current_time_ms().unwrap_or(0));
        }
        let cand = "ef".repeat(32);
        let sig = attest_sig_hex(&sk, h, 0, "vo1", &cand, None);
        route_cluster_stub(
            &app,
            "peer-b",
            PeerWireMsg::ClusterAttest {
                msg: ClusterAttestWire {
                    height: h,
                    round: 0,
                    vote_object: "vo1".to_string(),
                    candidate_hash: cand,
                    signature: sig,
                    candidate_ref: None,
                    attester_tip_height: None,
                },
            },
        )
        .await;
        let hs = handshake_read_traced(&app, "peer_session_mod").await;
        let got = hs
            .cluster_attest
            .rounds
            .get(&(h, 0))
            .map(|x| x.attesters.len())
            .unwrap_or(0);
        assert_eq!(got, 1);
    }

    #[tokio::test]
    async fn cluster_attest_cref_sig_ok() {
        let mut app = app_from_dev_net();
        app.cluster_cfg.enabled = true;
        app.cluster_cfg.role = crate::handshake::ClusterRole::Proposer;
        app.cluster_cfg.members = vec!["node-a".to_string(), "node-b".to_string()];
        app.cluster_cfg.quorum_n = 2;
        app.cluster_cfg.quorum_k = 1;
        let h = app.inner.read().await.chain.tip_h().saturating_add(1);
        let sk = SigningKey::from_bytes(&[6u8; 32]);
        let cref = "epoch:seg42";
        let cand = "ee".repeat(32);
        {
            let mut hs = handshake_write_traced(&app, "cluster_route_stub").await;
            hs.trusted_peers.insert(
                "peer-b".to_string(),
                crate::transport::TrustedPeer {
                    node_id: "peer-b".to_string(),
                    cluster_id: "cluster-a".to_string(),
                    pubkey: sk.verifying_key().to_bytes(),
                    domain_hi: app.identity.cluster_domain_hi,
                    instance_id: Some("node-b".to_string()),
                    cluster_attest_enabled: true,
                    cluster_role: crate::handshake::ClusterRole::Attester,
                },
            );
            let round = hs.cluster_attest.rounds.entry((h, 0)).or_default();
            round.vote_object = "vo-cref".to_string();
            round.candidate_hash = cand.clone();
            round.candidate_ref = Some(cref.to_string());
            round.proposer_id = Some("node-a".to_string());
            round.propose_opened_at_ms = Some(crate::current_time_ms().unwrap_or(0));
        }
        let sig = attest_sig_hex(&sk, h, 0, "vo-cref", &cand, Some(cref));
        route_cluster_stub(
            &app,
            "peer-b",
            PeerWireMsg::ClusterAttest {
                msg: ClusterAttestWire {
                    height: h,
                    round: 0,
                    vote_object: "vo-cref".to_string(),
                    candidate_hash: cand,
                    signature: sig,
                    candidate_ref: Some(cref.to_string()),
                    attester_tip_height: None,
                },
            },
        )
        .await;
        let hs = handshake_read_traced(&app, "peer_session_mod").await;
        let got = hs
            .cluster_attest
            .rounds
            .get(&(h, 0))
            .map(|x| x.attesters.len())
            .unwrap_or(0);
        assert_eq!(got, 1);
    }

    #[tokio::test]
    async fn cluster_attest_cref_sig_mismatch() {
        let mut app = app_from_dev_net();
        app.cluster_cfg.enabled = true;
        app.cluster_cfg.role = crate::handshake::ClusterRole::Proposer;
        app.cluster_cfg.members = vec!["node-a".to_string(), "node-b".to_string()];
        app.cluster_cfg.quorum_n = 2;
        app.cluster_cfg.quorum_k = 1;
        let h = app.inner.read().await.chain.tip_h().saturating_add(1);
        let sk = SigningKey::from_bytes(&[5u8; 32]);
        let cand = "dd".repeat(32);
        {
            let mut hs = handshake_write_traced(&app, "cluster_route_stub").await;
            hs.trusted_peers.insert(
                "peer-b".to_string(),
                crate::transport::TrustedPeer {
                    node_id: "peer-b".to_string(),
                    cluster_id: "cluster-a".to_string(),
                    pubkey: sk.verifying_key().to_bytes(),
                    domain_hi: app.identity.cluster_domain_hi,
                    instance_id: Some("node-b".to_string()),
                    cluster_attest_enabled: true,
                    cluster_role: crate::handshake::ClusterRole::Attester,
                },
            );
            let round = hs.cluster_attest.rounds.entry((h, 0)).or_default();
            round.vote_object = "vo-z".to_string();
            round.candidate_hash = cand.clone();
            round.candidate_ref = Some("must-bind".to_string());
            round.proposer_id = Some("node-a".to_string());
            round.propose_opened_at_ms = Some(crate::current_time_ms().unwrap_or(0));
        }
        // Wire sends ref (binding matches) but signature used empty ref line — must fail verify.
        let bad_sig = attest_sig_hex(&sk, h, 0, "vo-z", &cand, None);
        route_cluster_stub(
            &app,
            "peer-b",
            PeerWireMsg::ClusterAttest {
                msg: ClusterAttestWire {
                    height: h,
                    round: 0,
                    vote_object: "vo-z".to_string(),
                    candidate_hash: cand,
                    signature: bad_sig,
                    candidate_ref: Some("must-bind".to_string()),
                    attester_tip_height: None,
                },
            },
        )
        .await;
        let hs = handshake_read_traced(&app, "peer_session_mod").await;
        let got = hs
            .cluster_attest
            .rounds
            .get(&(h, 0))
            .map(|x| x.attesters.len())
            .unwrap_or(0);
        assert_eq!(got, 0);
    }

    #[tokio::test]
    async fn cluster_prop_auto_ack() {
        let mut app = app_from_dev_net();
        app.cluster_cfg.enabled = true;
        app.cluster_cfg.role = crate::handshake::ClusterRole::Attester;
        app.cluster_cfg.members = vec!["node-a".to_string(), "node-b".to_string()];
        app.cluster_cfg.quorum_n = 2;
        app.cluster_cfg.quorum_k = 1;
        app.node_instance_id = "node-b".to_string();
        let h = app.inner.read().await.chain.tip_h().saturating_add(1);
        {
            let mut hs = handshake_write_traced(&app, "cluster_route_stub").await;
            hs.trusted_peers.insert(
                "peer-a".to_string(),
                crate::transport::TrustedPeer {
                    node_id: "peer-a".to_string(),
                    cluster_id: "cluster-a".to_string(),
                    pubkey: SigningKey::from_bytes(&[4u8; 32])
                        .verifying_key()
                        .to_bytes(),
                    domain_hi: app.identity.cluster_domain_hi,
                    instance_id: Some("node-a".to_string()),
                    cluster_attest_enabled: true,
                    cluster_role: crate::handshake::ClusterRole::Proposer,
                },
            );
        }
        let cand = "fa".repeat(32);
        let msg = PeerWireMsg::ClusterPropose {
            msg: ClusterProposeWire {
                height: h,
                round: 0,
                vote_object: "vo-auto".to_string(),
                candidate_hash: cand.clone(),
                candidate_ref: None,
                tail_blocks: Vec::new(),
            },
        };
        let out = route_cluster_stub(&app, "peer-a", msg).await;
        let Some(ack) = out else {
            panic!("expected local attester ack");
        };
        assert_eq!(ack.height, h);
        assert_eq!(ack.round, 0);
        assert_eq!(ack.vote_object, "vo-auto");
        assert_eq!(ack.candidate_hash, cand);
        let local_pk = local_hello_signing_key(&app.identity)
            .verifying_key()
            .to_bytes();
        let sig_msg = cluster_sig_msg(h, 0, &ack.vote_object, &ack.candidate_hash, None);
        assert!(verify_attest_sig(&ack.signature, &local_pk, &sig_msg));
    }

    #[tokio::test]
    async fn cluster_prop_tail_modes() {
        let mut app = app_from_dev_net();
        app.cluster_cfg.enabled = true;
        app.cluster_cfg.role = crate::handshake::ClusterRole::Proposer;
        app.cluster_cfg.members = vec!["node-a".to_string(), "node-b".to_string()];
        app.cluster_cfg.quorum_n = 2;
        app.cluster_cfg.quorum_k = 1;
        app.node_instance_id = "node-a".to_string();
        {
            let mut g = app.inner.write().await;
            g.chain.seal(Vec::new()).expect("seed tail block");
        }

        let lean = mk_cluster_prop(&app, None).await.expect("lean propose");
        assert!(lean.tail_blocks.is_empty());
        assert_eq!(lean.round, 0);
        assert!(!lean.vote_object.is_empty());
        assert!(!lean.candidate_hash.is_empty());

        app.cluster_cfg.full_blocks = true;
        let full = mk_cluster_prop(&app, None).await.expect("full propose");
        assert!(!full.tail_blocks.is_empty());
        assert!(full.tail_blocks.iter().all(|blk| blk.block.is_some()));
    }

    #[tokio::test]
    async fn cluster_prop_mirror_send() {
        let mut app = app_from_dev_net();
        app.cluster_cfg.enabled = true;
        app.cluster_cfg.role = crate::handshake::ClusterRole::Proposer;
        app.cluster_cfg.members = vec!["node-a".to_string(), "node-b".to_string()];
        app.cluster_cfg.quorum_n = 2;
        app.cluster_cfg.quorum_k = 1;
        app.node_instance_id = "node-a".to_string();
        let mut remote = remote_hello(&app, "peer-b").await;
        remote.capabilities.cluster_attest_enabled = true;
        remote.capabilities.cluster_role = crate::handshake::ClusterRole::Attester;
        remote.capabilities.node_instance_id = Some("node-b".to_string());
        let (mut stream, _peer) = test_stream().await;
        let cfg = TransportConfig::default();
        send_cluster_prop(&app, &cfg, &mut stream, &remote)
            .await
            .expect("send cluster propose");
        let next_h = app.inner.read().await.chain.tip_h().saturating_add(1);
        let hs = handshake_read_traced(&app, "peer_session_mod").await;
        let round = hs.cluster_attest.rounds.get(&(next_h, 0)).expect("round");
        assert_eq!(round.proposer_id.as_deref(), Some("node-a"));
        assert!(!round.vote_object.is_empty());
        assert!(!round.candidate_hash.is_empty());
    }

    #[tokio::test]
    async fn cluster_prop_skips_non_lead() {
        let mut app = app_from_dev_net();
        app.cluster_cfg.enabled = true;
        app.cluster_cfg.role = crate::handshake::ClusterRole::Proposer;
        app.cluster_cfg.members = vec!["node-a".to_string(), "node-b".to_string()];
        app.cluster_cfg.quorum_n = 2;
        app.cluster_cfg.quorum_k = 1;
        app.node_instance_id = "node-a".to_string();
        {
            let mut g = app.inner.write().await;
            g.chain.st.active_validator_indices = vec![1u16];
        }
        let mut remote = remote_hello(&app, "peer-b").await;
        remote.capabilities.cluster_attest_enabled = true;
        remote.capabilities.cluster_role = crate::handshake::ClusterRole::Attester;
        remote.capabilities.node_instance_id = Some("node-b".to_string());
        let (mut stream, _peer) = test_stream().await;
        let cfg = TransportConfig::default();
        send_cluster_prop(&app, &cfg, &mut stream, &remote)
            .await
            .expect("send cluster propose");
        let next_h = app.inner.read().await.chain.tip_h().saturating_add(1);
        let hs = handshake_read_traced(&app, "peer_session_mod").await;
        assert!(
            hs.cluster_attest.rounds.get(&(next_h, 0)).is_none(),
            "non-leader proposer must not open round"
        );
    }
}
