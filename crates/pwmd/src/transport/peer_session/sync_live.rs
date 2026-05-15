//! Same-shard sync v1 live flow: tip announce, header align, block fetch, and safe apply.
//! Short tail (small lag) prefers live hdr/blk; epoch catch-up activates at deep lag or mid-cup retry.

use super::super::*;
use super::{
    write_wire_msg, PeerWireMsg, SyncBlockWire, SyncCatchupChunkWire, SyncHeaderWire, SyncWireHdr,
};
use crate::handshake::NodeHello;
use crate::handshake::SealRole;
use crate::lifecycle::periodic_snap_finish;
use crate::lifecycle::{autosnap_hit, STANDBY_SYNC_FLUSH_IV};
use crate::snapshot::epoch::{epoch_idx, epoch_range};
use crate::snapshot::incremental::{
    load_block_at_height, load_consecutive_blocks_from_epochs, load_hash_scan_blocks,
};
use crate::snapshot::SealPersistMode;
use crate::transport::handshake_state::{HandshakeState, SyncPeerState};
use pwm_core::block::{hdr_hash, txs_root, Block};
use pwm_core::{digest, TAIL_BLOCK_CAP};
use std::collections::VecDeque;
use std::sync::atomic::Ordering;

const SYNC_HDR_REQ_CAP: u16 = 128;
const SYNC_BLK_REQ_CAP: u16 = 32;
const SYNC_INF_CAP: u8 = 8;
const SYNC_PEND_CAP: usize = 512;
const SYNC_PEER_CAP: usize = 64;
const SYNC_CUP_LAG_MIN: u64 = 256;
/// Below this gap-to-tip, never start epoch catch-up from `live_stall` alone; use live headers/blocks.
const SYNC_CUP_TAIL_MAX: u64 = 32;
const SYNC_CUP_WIN_CAP: u64 = 4_096;
const SYNC_CUP_CHUNK_CAP: usize = 32;
const SYNC_CUP_TRY_CAP: u8 = 3;
const SYNC_PROG_MIN_MS: u64 = 7_000;
/// Quiet console during healthy live short-tail: peer tip advances by 1, no epoch CUP in flight.
const SYNC_PROG_TAIL_MS: u64 = 60_000;

pub(super) struct TipDivergence {
    pub(super) local_h: u64,
    pub(super) local_hash: String,
    pub(super) peer_h: u64,
    pub(super) peer_hash: String,
}

fn chain_hash_at(
    tip_h: u64,
    tip_hash_hex: &str,
    blocks: &VecDeque<Block>,
    height: u64,
) -> Option<String> {
    if height == tip_h {
        return Some(tip_hash_hex.to_string());
    }
    blocks
        .iter()
        .rev()
        .find(|b| b.hdr.height == height)
        .map(|b| hex::encode(hdr_hash(&b.hdr)))
}

fn now_ms() -> u64 {
    current_time_ms().unwrap_or(0)
}

fn has_hash(rows: &VecDeque<(u64, String)>, hash: &str) -> bool {
    rows.iter().any(|(_, got)| got == hash)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SyncProgSnap {
    goal: u64,
    pct: u8,
    rem: u64,
}

fn sync_prog_snap(local_h: u64, peer_tip_h: u64, persisted_h: u64) -> Option<SyncProgSnap> {
    let goal = if peer_tip_h > 0 {
        peer_tip_h
    } else {
        local_h.max(persisted_h)
    };
    if goal == 0 {
        return None;
    }
    if local_h >= goal {
        return Some(SyncProgSnap {
            goal,
            pct: 100,
            rem: 0,
        });
    }
    let rem = goal.saturating_sub(local_h);
    let mut pct = local_h.saturating_mul(100).saturating_div(goal).min(100) as u8;
    if rem > 0 && pct == 100 {
        pct = 99;
    }
    Some(SyncProgSnap { goal, pct, rem })
}

/// Live "short tail": following peer close to tip without epoch CUP — suppress noisy `Sync progress` on Standby only.
fn sync_prog_tail_quiet(cup_active: bool, peer_tip_h: u64, local_h: u64) -> bool {
    let tip_lag = peer_tip_h.saturating_sub(local_h);
    !cup_active && peer_tip_h > 0 && tip_lag < SYNC_CUP_TAIL_MAX
}

fn sync_prog_tick(
    st: &mut SyncPeerState,
    now_ms: u64,
    local_h: u64,
    peer_tip_h: u64,
    persisted_h: u64,
) -> Option<SyncProgSnap> {
    let Some(snap) = sync_prog_snap(local_h, peer_tip_h, persisted_h) else {
        st.sync_log_done = false;
        return None;
    };
    // Same tip goal + already printed 100%: skip even if `sync_log_done` was reset (multi-path / genesis).
    if snap.rem == 0 && snap.pct == 100 && st.sync_pct100_goal == Some(snap.goal) {
        return None;
    }
    let tail_quiet = sync_prog_tail_quiet(st.cup_active, peer_tip_h, local_h);
    let min_ms = if tail_quiet {
        SYNC_PROG_TAIL_MS
    } else {
        SYNC_PROG_MIN_MS
    };
    let time_ok = st.sync_log_ms == 0 || now_ms.saturating_sub(st.sync_log_ms) >= min_ms;
    let pct_ok = st
        .sync_log_pct
        .map(|prev| snap.pct.saturating_sub(prev) >= 1)
        .unwrap_or(true);
    let quiet_goal_bump = tail_quiet
        && snap.rem > 0
        && st.sync_log_done
        && st.sync_pct100_goal.map_or(false, |g| snap.goal > g);
    let lag_resume = snap.rem > 0 && st.sync_log_done && !tail_quiet;
    let done_now = snap.rem == 0 && !st.sync_log_done;
    let rem_progress = snap.rem > 0 && time_ok && (pct_ok || lag_resume || quiet_goal_bump);
    if !(done_now || rem_progress) {
        return None;
    }
    st.sync_log_ms = now_ms;
    st.sync_log_pct = Some(snap.pct);
    st.sync_log_done = snap.rem == 0;
    if snap.rem == 0 && snap.pct == 100 {
        st.sync_pct100_goal = Some(snap.goal);
    }
    Some(snap)
}

async fn maybe_log_sync_prog(
    app: &App,
    node_id: &str,
    local_h: u64,
    peer_tip_h: u64,
    persisted_h: u64,
) {
    let now = now_ms();
    let (snap, emit_progress_line) = {
        let mut hs = app.handshake.write().await;
        let st = peer_sync(&mut hs, node_id);
        let tick = sync_prog_tick(st, now, local_h, peer_tip_h, persisted_h);
        let Some(snap) = tick else {
            return;
        };
        let tail_quiet = sync_prog_tail_quiet(st.cup_active, peer_tip_h, local_h);
        let suppress_short_tail_standby = matches!(app.seal_role, SealRole::Standby) && tail_quiet;
        (snap, !suppress_short_tail_standby)
    };
    if emit_progress_line {
        info!(
            target: "pwmd::sync",
            "Sync progress {}% rem={} goal={} mem={}/{} disk={}/{}",
            snap.pct,
            snap.rem,
            snap.goal,
            local_h,
            snap.goal,
            persisted_h,
            snap.goal
        );
    }
}

fn cup_backoff_ms(cfg: &TransportConfig, cup_try: u8) -> u64 {
    let step = cfg.heartbeat_interval_ms.max(200);
    let shift = (cup_try as u32).min(6);
    step.saturating_mul(1u64 << shift)
        .min(cfg.retry_max_ms.max(step))
}

fn cup_req_range(local_h: u64, head_h: u64) -> Result<Option<(u64, u64)>, String> {
    if head_h <= local_h {
        return Ok(None);
    }
    let from_h = local_h.saturating_add(1);
    let lag = head_h.saturating_sub(local_h);
    let lag_span = lag.min(SYNC_CUP_WIN_CAP).saturating_sub(1);
    let max_to_h = from_h.saturating_add(lag_span);
    let epoch_last_h = epoch_range(epoch_idx(from_h)?).last_h;
    let to_h = max_to_h.min(epoch_last_h);
    Ok(Some((from_h, to_h)))
}

fn cup_fail(hs: &mut HandshakeState, reason: &str) {
    hs.transport.snapshot.sync_cup_fail_total =
        hs.transport.snapshot.sync_cup_fail_total.saturating_add(1);
    increment_string_u64_bucket(&mut hs.transport.snapshot.sync_cup_fail_reason, reason);
}

pub(super) fn sync_caps(remote: &NodeHello) -> (u16, u16) {
    let max_hdr = remote
        .capabilities
        .sync_profile
        .as_ref()
        .map(|x| x.max_headers_per_msg)
        .unwrap_or(0);
    let max_blk = remote
        .capabilities
        .sync_profile
        .as_ref()
        .map(|x| x.max_blocks_per_msg)
        .unwrap_or(0);
    (max_hdr, max_blk)
}

pub(super) async fn send_sync_tip(
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
    *seq_no = seq_no.saturating_add(1);
    let now_ms = current_time_ms().unwrap_or(0);
    let (head_h, head_hash, finalized_h, finalized_hash) = {
        let g = app.inner.read().await;
        let head_h = g.chain.tip_h();
        let head_hash = hex::encode(g.chain.tip_hash());
        // Use penultimate block as safest settled anchor approximation.
        let finalized_h = head_h.saturating_sub(1);
        let finalized_hash = if finalized_h == 0 {
            None
        } else {
            chain_hash_at(head_h, &head_hash, &g.chain.blocks, finalized_h)
        };
        (head_h, head_hash, finalized_h, finalized_hash)
    };
    write_wire_msg(
        stream,
        &PeerWireMsg::SyncTipAnnounce {
            hdr: SyncWireHdr {
                shard_id: app.identity.cluster_domain_hi,
                peer_session_id: app.identity.node_id.clone(),
                seq_no: *seq_no,
                timestamp_ms: now_ms,
            },
            head_height: head_h,
            head_hash,
            finalized_height: finalized_h,
            finalized_hash,
        },
        cfg.heartbeat_timeout_ms,
    )
    .await
}

fn peer_sync<'a>(hs: &'a mut HandshakeState, node_id: &str) -> &'a mut SyncPeerState {
    if !hs.sync_live.peers.contains_key(node_id) && hs.sync_live.peers.len() >= SYNC_PEER_CAP {
        if let Some(victim) = hs.sync_live.peers.keys().next().cloned() {
            hs.sync_live.peers.remove(&victim);
        }
    }
    hs.sync_live.peers.entry(node_id.to_string()).or_default()
}

async fn send_nack(
    app: &App,
    cfg: &TransportConfig,
    stream: &mut tokio::net::TcpStream,
    reason_code: &str,
    seq_no: &mut u64,
) -> Result<(), String> {
    *seq_no = seq_no.saturating_add(1);
    let now_ms = current_time_ms().unwrap_or(0);
    write_wire_msg(
        stream,
        &PeerWireMsg::SyncNack {
            hdr: SyncWireHdr {
                shard_id: app.identity.cluster_domain_hi,
                peer_session_id: app.identity.node_id.clone(),
                seq_no: *seq_no,
                timestamp_ms: now_ms,
            },
            reason_code: reason_code.to_string(),
            retry_after_ms: cfg.heartbeat_interval_ms.min(u32::MAX as u64) as u32,
        },
        cfg.heartbeat_timeout_ms,
    )
    .await
}

async fn ask_hdr(
    app: &App,
    cfg: &TransportConfig,
    stream: &mut tokio::net::TcpStream,
    node_id: &str,
    from_h: u64,
    lim: u16,
    seq_no: &mut u64,
) -> Result<(), String> {
    let mut req_lim = lim.min(SYNC_HDR_REQ_CAP).max(1);
    {
        let mut hs = app.handshake.write().await;
        {
            let st = peer_sync(&mut hs, node_id);
            if st.wait_hdr_from == Some(from_h) && st.in_hdr > 0 {
                return Ok(());
            }
            if st.in_hdr >= SYNC_INF_CAP {
                return Ok(());
            }
            st.wait_hdr_from = Some(from_h);
            st.wait_hdr_lim = req_lim;
            st.in_hdr = st.in_hdr.saturating_add(1);
            req_lim = st.wait_hdr_lim;
        }
        hs.transport.snapshot.sync_hdr_req_total =
            hs.transport.snapshot.sync_hdr_req_total.saturating_add(1);
    }
    *seq_no = seq_no.saturating_add(1);
    let now_ms = current_time_ms().unwrap_or(0);
    write_wire_msg(
        stream,
        &PeerWireMsg::SyncHeadersReq {
            hdr: SyncWireHdr {
                shard_id: app.identity.cluster_domain_hi,
                peer_session_id: app.identity.node_id.clone(),
                seq_no: *seq_no,
                timestamp_ms: now_ms,
            },
            from_height: from_h,
            limit: req_lim,
        },
        cfg.heartbeat_timeout_ms,
    )
    .await
}

async fn ask_blk(
    app: &App,
    cfg: &TransportConfig,
    stream: &mut tokio::net::TcpStream,
    node_id: &str,
    blk_cap: u16,
    seq_no: &mut u64,
) -> Result<(), String> {
    let mut want: Vec<String> = Vec::new();
    let mut want_h: Vec<u64> = Vec::new();
    let lim = blk_cap.min(SYNC_BLK_REQ_CAP).max(1) as usize;
    {
        let mut hs = app.handshake.write().await;
        let st = peer_sync(&mut hs, node_id);
        if st.in_blk >= SYNC_INF_CAP || !st.wait_blk.is_empty() {
            return Ok(());
        }
        while want.len() < lim {
            let Some((height, hash)) = st.pend_blk.pop_front() else {
                break;
            };
            st.wait_blk.push_back((height, hash.clone()));
            want_h.push(height);
            want.push(hash);
        }
        if want.is_empty() {
            return Ok(());
        }
        st.in_blk = st.in_blk.saturating_add(1);
        hs.transport.snapshot.sync_blk_req_total =
            hs.transport.snapshot.sync_blk_req_total.saturating_add(1);
    }
    *seq_no = seq_no.saturating_add(1);
    let now_ms = current_time_ms().unwrap_or(0);
    write_wire_msg(
        stream,
        &PeerWireMsg::SyncBlocksReq {
            hdr: SyncWireHdr {
                shard_id: app.identity.cluster_domain_hi,
                peer_session_id: app.identity.node_id.clone(),
                seq_no: *seq_no,
                timestamp_ms: now_ms,
            },
            block_hashes: want,
            block_heights: Some(want_h),
        },
        cfg.heartbeat_timeout_ms,
    )
    .await
}

/// After successful live apply: schedule the next hdr round for short tip lag when blk queues are idle.
/// Avoids waiting for another `on_tip` tick to close a small gap (priority live path vs background CUP).
async fn live_tail_pull_hdr(
    app: &App,
    cfg: &TransportConfig,
    stream: &mut tokio::net::TcpStream,
    node_id: &str,
    hdr_cap: u16,
    seq_no: &mut u64,
    local_h: u64,
    tip_h: u64,
) -> Result<(), String> {
    let lag = tip_h.saturating_sub(local_h);
    if lag == 0 || lag >= SYNC_CUP_TAIL_MAX {
        return Ok(());
    }
    let (cup, pend_empty, wait_empty, in_hdr) = {
        let hs = app.handshake.read().await;
        let Some(st) = hs.sync_live.peers.get(node_id) else {
            return Ok(());
        };
        (
            st.cup_active,
            st.pend_blk.is_empty(),
            st.wait_blk.is_empty(),
            st.in_hdr,
        )
    };
    if cup || !pend_empty || !wait_empty || in_hdr > 0 {
        return Ok(());
    }
    let req_lim = hdr_cap.min(SYNC_HDR_REQ_CAP).max(1);
    ask_hdr(
        app,
        cfg,
        stream,
        node_id,
        local_h.saturating_add(1),
        req_lim,
        seq_no,
    )
    .await
}

async fn send_cup_req(
    app: &App,
    cfg: &TransportConfig,
    stream: &mut tokio::net::TcpStream,
    node_id: &str,
    from_h: u64,
    to_h: u64,
    seq_no: &mut u64,
) -> Result<(), String> {
    let local_hash = {
        let g = app.inner.read().await;
        hex::encode(g.chain.tip_hash())
    };
    let eid = epoch_idx(from_h)?;
    {
        let mut hs = app.handshake.write().await;
        let st = peer_sync(&mut hs, node_id);
        st.cup_active = true;
        st.cup_epoch = eid;
        st.cup_from = from_h;
        st.cup_to = to_h;
        st.cup_next_h = from_h;
        st.cup_next_ix = 0;
        st.cup_prev_hash = local_hash.clone();
        st.cup_target_h = st.tip_h.max(to_h);
        st.cup_next_ms = 0;
        hs.transport.snapshot.sync_cup_start_total =
            hs.transport.snapshot.sync_cup_start_total.saturating_add(1);
    }
    *seq_no = seq_no.saturating_add(1);
    if let Err(err) = write_wire_msg(
        stream,
        &PeerWireMsg::SyncCatchupReq {
            hdr: SyncWireHdr {
                shard_id: app.identity.cluster_domain_hi,
                peer_session_id: app.identity.node_id.clone(),
                seq_no: *seq_no,
                timestamp_ms: now_ms(),
            },
            start_height: from_h,
            end_height: to_h,
            epoch_id: eid,
            anchor_hash: local_hash,
        },
        cfg.heartbeat_timeout_ms,
    )
    .await
    {
        let mut hs = app.handshake.write().await;
        cup_fail(&mut hs, "req_write");
        let st = peer_sync(&mut hs, node_id);
        st.live_stall = st.live_stall.saturating_add(1);
        st.cup_try = st.cup_try.saturating_add(1);
        st.cup_next_ms = now_ms().saturating_add(cup_backoff_ms(cfg, st.cup_try));
        cup_clear(st);
        return Err(err);
    }
    info!(
        target: "pwmd::peer",
        "peer sync catchup start node_id={} epoch_id={} range={}..={}",
        node_id,
        eid,
        from_h,
        to_h
    );
    Ok(())
}

async fn maybe_start_cup(
    app: &App,
    cfg: &TransportConfig,
    stream: &mut tokio::net::TcpStream,
    node_id: &str,
    head_h: u64,
    can_cup: bool,
    seq_no: &mut u64,
) -> Result<bool, String> {
    if !can_cup {
        let mut hs = app.handshake.write().await;
        hs.transport.snapshot.sync_cup_drop_total =
            hs.transport.snapshot.sync_cup_drop_total.saturating_add(1);
        cup_fail(&mut hs, "feature_mismatch");
        return Ok(false);
    }
    let (local_h, next_ms, cup_on, cup_try) = {
        let g = app.inner.read().await;
        let local_h = g.chain.tip_h();
        drop(g);
        let hs = app.handshake.read().await;
        let st = hs.sync_live.peers.get(node_id);
        (
            local_h,
            st.map(|x| x.cup_next_ms).unwrap_or(0),
            st.map(|x| x.cup_active).unwrap_or(false),
            st.map(|x| x.cup_try).unwrap_or(0),
        )
    };
    if cup_on || now_ms() < next_ms || head_h <= local_h || cup_try > SYNC_CUP_TRY_CAP {
        return Ok(cup_on);
    }
    let Some((from_h, to_h)) = cup_req_range(local_h, head_h)? else {
        return Ok(false);
    };
    if let Err(err) = send_cup_req(app, cfg, stream, node_id, from_h, to_h, seq_no).await {
        warn!(
            target: "pwmd::peer",
            "peer sync catchup start failed node_id={} error={}",
            node_id,
            err
        );
        return Ok(false);
    }
    Ok(true)
}

pub(super) async fn on_tip(
    app: &App,
    cfg: &TransportConfig,
    stream: &mut tokio::net::TcpStream,
    node_id: &str,
    head_h: u64,
    head_hash: &str,
    finalized_h: u64,
    finalized_hash: Option<&str>,
    hdr_cap: u16,
    can_cup: bool,
    seq_no: &mut u64,
) -> Result<Option<TipDivergence>, String> {
    let (local_h, local_hash, local_finalized_h) = {
        let g = app.inner.read().await;
        let local_h = g.chain.tip_h();
        (
            local_h,
            hex::encode(g.chain.tip_hash()),
            local_h.saturating_sub(1),
        )
    };
    let (live_stall, cup_on) = {
        let mut hs = app.handshake.write().await;
        hs.transport.snapshot.sync_tip_seen_total =
            hs.transport.snapshot.sync_tip_seen_total.saturating_add(1);
        let st = peer_sync(&mut hs, node_id);
        st.tip_h = st.tip_h.max(head_h);
        st.tip_hash = Some(head_hash.to_string());
        (st.live_stall, st.cup_active && now_ms() >= st.cup_next_ms)
    };
    if head_h < local_h {
        return Ok(None);
    }
    let lag = head_h - local_h;
    let persisted_h = app.last_snapshot_height.load(Ordering::Acquire);
    maybe_log_sync_prog(app, node_id, local_h, head_h, persisted_h).await;
    let demoted = {
        let mut hs = app.handshake.write().await;
        try_demote_cup_tail(&mut hs, node_id, lag)
    };
    if demoted {
        info!(
            target: "pwmd::peer",
            "peer sync cup_demoted_short_tail node_id={} tip_lag={} head_h={} local_h={}",
            node_id,
            lag,
            head_h,
            local_h
        );
    }
    if lag == 0 {
        if let Some(peer_fin_hash) = finalized_hash {
            if finalized_h > 0 && finalized_h <= local_h {
                let local_fin_hash = {
                    let g = app.inner.read().await;
                    let tip_h = g.chain.tip_h();
                    let tip_hash = hex::encode(g.chain.tip_hash());
                    chain_hash_at(tip_h, &tip_hash, &g.chain.blocks, finalized_h)
                };
                if let Some(local_fin_hash) = local_fin_hash {
                    if local_fin_hash != peer_fin_hash {
                        return Ok(Some(TipDivergence {
                            local_h: finalized_h,
                            local_hash: local_fin_hash,
                            peer_h: finalized_h,
                            peer_hash: peer_fin_hash.to_string(),
                        }));
                    }
                    return Ok(None);
                }
            }
        }
        if head_hash != local_hash {
            if finalized_hash.is_some() && finalized_h < head_h && local_finalized_h < head_h {
                return Ok(None);
            }
            return Ok(Some(TipDivergence {
                local_h,
                local_hash,
                peer_h: head_h,
                peer_hash: head_hash.to_string(),
            }));
        }
        return Ok(None);
    }
    info!(
        target: "pwmd::peer",
        "peer sync on_tip lag node_id={} local_h={} head_h={} lag={} persisted_h={} live_stall={} cup_on={} can_cup={}",
        node_id,
        local_h,
        head_h,
        lag,
        persisted_h,
        live_stall,
        cup_on,
        can_cup
    );
    // `cup_on`: mid-catch-up retry timer — always allow `maybe_start_cup` to run its early exits.
    // Short tail (lag < 256): stay on the live hdr/blk path; do not arm CUP from `live_stall` alone
    // (CUP remains the deep / epoch background path; retries only via `cup_on`).
    let cup_req = cup_on || lag >= SYNC_CUP_LAG_MIN;
    let cup_started = if cup_req {
        maybe_start_cup(app, cfg, stream, node_id, head_h, can_cup, seq_no).await?
    } else {
        false
    };
    if cup_started {
        info!(
            target: "pwmd::peer",
            "peer sync on_tip cup_started node_id={} lag={} live_stall={} cup_on={} can_cup={}",
            node_id,
            lag,
            live_stall,
            cup_on,
            can_cup
        );
        return Ok(None);
    }
    if cup_req {
        let cup_try = {
            let hs = app.handshake.read().await;
            hs.sync_live
                .peers
                .get(node_id)
                .map(|x| x.cup_try)
                .unwrap_or(0)
        };
        info!(
            target: "pwmd::peer",
            "peer sync on_tip cup_skipped node_id={} lag={} cup_try={} live_stall={} cup_on={} can_cup={}",
            node_id,
            lag,
            cup_try,
            live_stall,
            cup_on,
            can_cup
        );
    }
    let req_lim = hdr_cap.min(SYNC_HDR_REQ_CAP).max(1);
    info!(
        target: "pwmd::peer",
        "peer sync on_tip live_hdr node_id={} next_from={} req_lim={} lag={}",
        node_id,
        local_h.saturating_add(1),
        req_lim,
        lag
    );
    ask_hdr(
        app,
        cfg,
        stream,
        node_id,
        local_h.saturating_add(1),
        req_lim,
        seq_no,
    )
    .await?;
    Ok(None)
}

pub(super) async fn on_hdr_req(
    app: &App,
    cfg: &TransportConfig,
    stream: &mut tokio::net::TcpStream,
    from_h: u64,
    limit: u16,
    hdr_cap: u16,
    seq_no: &mut u64,
) -> Result<(), String> {
    let hard = hdr_cap.min(SYNC_HDR_REQ_CAP).max(1);
    if from_h == 0 || limit == 0 || limit > hard {
        return send_nack(app, cfg, stream, "headers_limit", seq_no).await;
    }
    let req_lim = usize::from(limit.min(hard).max(1));
    let mut out = {
        let g = app.inner.read().await;
        let mut v = Vec::new();
        for blk in g.chain.blocks.iter().filter(|b| b.hdr.height >= from_h) {
            if v.len() >= req_lim {
                break;
            }
            if !v.is_empty() {
                let prev_h = v.last().map(|x: &SyncHeaderWire| x.height).unwrap_or(0);
                if blk.hdr.height != prev_h.saturating_add(1) {
                    break;
                }
            }
            v.push(SyncHeaderWire {
                height: blk.hdr.height,
                hash: hex::encode(hdr_hash(&blk.hdr)),
                prev_hash: hex::encode(blk.hdr.prev_hash),
            });
        }
        v
    };
    if out.is_empty() || out.first().map(|x| x.height) != Some(from_h) {
        if let Some(path) = app.data_file.as_ref() {
            if let Ok(disk_rows) = load_consecutive_blocks_from_epochs(path, from_h, req_lim) {
                if !disk_rows.is_empty() && disk_rows.first().map(|x| x.hdr.height) == Some(from_h)
                {
                    out = disk_rows
                        .into_iter()
                        .map(|blk| SyncHeaderWire {
                            height: blk.hdr.height,
                            hash: hex::encode(hdr_hash(&blk.hdr)),
                            prev_hash: hex::encode(blk.hdr.prev_hash),
                        })
                        .collect();
                }
            }
        }
    }
    if out.is_empty() || out.first().map(|x| x.height) != Some(from_h) {
        return send_nack(app, cfg, stream, "headers_range", seq_no).await;
    }
    *seq_no = seq_no.saturating_add(1);
    let now_ms = current_time_ms().unwrap_or(0);
    write_wire_msg(
        stream,
        &PeerWireMsg::SyncHeadersBatch {
            hdr: SyncWireHdr {
                shard_id: app.identity.cluster_domain_hi,
                peer_session_id: app.identity.node_id.clone(),
                seq_no: *seq_no,
                timestamp_ms: now_ms,
            },
            headers: out,
        },
        cfg.heartbeat_timeout_ms,
    )
    .await?;
    let mut hs = app.handshake.write().await;
    hs.transport.snapshot.sync_hdr_resp_total =
        hs.transport.snapshot.sync_hdr_resp_total.saturating_add(1);
    Ok(())
}

pub(super) async fn on_hdr_batch(
    app: &App,
    cfg: &TransportConfig,
    stream: &mut tokio::net::TcpStream,
    node_id: &str,
    headers: Vec<SyncHeaderWire>,
    hdr_cap: u16,
    blk_cap: u16,
    seq_no: &mut u64,
) -> Result<(), String> {
    let hard = hdr_cap.min(SYNC_HDR_REQ_CAP).max(1) as usize;
    if headers.is_empty() || headers.len() > hard {
        return Ok(());
    }
    {
        let hs = app.handshake.read().await;
        if hs
            .sync_live
            .peers
            .get(node_id)
            .map(|x| x.cup_active)
            .unwrap_or(false)
        {
            return Ok(());
        }
    }
    let (from_h, req_lim, tip_h) = {
        let mut hs = app.handshake.write().await;
        hs.transport.snapshot.sync_hdr_resp_total =
            hs.transport.snapshot.sync_hdr_resp_total.saturating_add(1);
        let st = peer_sync(&mut hs, node_id);
        st.in_hdr = st.in_hdr.saturating_sub(1);
        let req = st.wait_hdr_from.take();
        (req, st.wait_hdr_lim.max(1), st.tip_h)
    };
    let Some(exp_h) = from_h else {
        return Ok(());
    };
    if headers.first().map(|x| x.height) != Some(exp_h) || headers.len() > req_lim as usize {
        let mut hs = app.handshake.write().await;
        hs.transport.snapshot.sync_fork_conflict_total = hs
            .transport
            .snapshot
            .sync_fork_conflict_total
            .saturating_add(1);
        let st = peer_sync(&mut hs, node_id);
        st.live_stall = st.live_stall.saturating_add(1);
        warn!(
            target: "pwmd::peer",
            "peer sync headers rejected node_id={} reason=continuity_start expected={} got={:?}",
            node_id,
            exp_h,
            headers.first().map(|x| x.height)
        );
        return Ok(());
    }
    let (local_h, mut prev_hash) = {
        let g = app.inner.read().await;
        (g.chain.tip_h(), hex::encode(g.chain.tip_hash()))
    };
    if exp_h != local_h.saturating_add(1) {
        let mut hs = app.handshake.write().await;
        let st = peer_sync(&mut hs, node_id);
        st.live_stall = st.live_stall.saturating_add(1);
        return Ok(());
    }
    let mut next_h = exp_h;
    let mut blk_rows = Vec::new();
    for hdr in headers.iter() {
        if hdr.height != next_h || hdr.prev_hash != prev_hash {
            let mut hs = app.handshake.write().await;
            hs.transport.snapshot.sync_fork_conflict_total = hs
                .transport
                .snapshot
                .sync_fork_conflict_total
                .saturating_add(1);
            let st = peer_sync(&mut hs, node_id);
            st.live_stall = st.live_stall.saturating_add(1);
            warn!(
                target: "pwmd::peer",
                "peer sync headers rejected node_id={} reason=continuity_break at_height={}",
                node_id,
                hdr.height
            );
            return Ok(());
        }
        prev_hash = hdr.hash.clone();
        next_h = next_h.saturating_add(1);
        blk_rows.push((hdr.height, hdr.hash.clone()));
    }
    {
        let mut hs = app.handshake.write().await;
        let st = peer_sync(&mut hs, node_id);
        for (height, hash) in blk_rows {
            if has_hash(&st.pend_blk, &hash) || has_hash(&st.wait_blk, &hash) {
                continue;
            }
            st.pend_blk.push_back((height, hash));
            while st.pend_blk.len() > SYNC_PEND_CAP {
                st.pend_blk.pop_front();
            }
        }
    }
    if next_h <= tip_h {
        ask_hdr(app, cfg, stream, node_id, next_h, req_lim, seq_no).await?;
    }
    ask_blk(app, cfg, stream, node_id, blk_cap, seq_no).await
}

pub(super) async fn on_blk_req(
    app: &App,
    cfg: &TransportConfig,
    stream: &mut tokio::net::TcpStream,
    block_hashes: Vec<String>,
    block_heights: Option<Vec<u64>>,
    blk_cap: u16,
    seq_no: &mut u64,
) -> Result<(), String> {
    let hard = blk_cap.min(SYNC_BLK_REQ_CAP).max(1) as usize;
    if block_hashes.is_empty() || block_hashes.len() > hard {
        return send_nack(app, cfg, stream, "blocks_limit", seq_no).await;
    }
    if block_heights
        .as_ref()
        .map(|x| x.len() != block_hashes.len())
        .unwrap_or(false)
    {
        return send_nack(app, cfg, stream, "blocks_range", seq_no).await;
    }
    let (tail, data_file) = {
        let g = app.inner.read().await;
        (g.chain.blocks.clone(), app.data_file.clone())
    };
    let mut out: Vec<Option<SyncBlockWire>> = vec![None; block_hashes.len()];
    let mut misses: Vec<usize> = Vec::new();
    for (ix, want) in block_hashes.iter().enumerate() {
        let Some(blk) = tail
            .iter()
            .find(|b| hex::encode(hdr_hash(&b.hdr)) == *want)
            .cloned()
        else {
            misses.push(ix);
            continue;
        };
        out[ix] = Some(SyncBlockWire {
            height: blk.hdr.height,
            hash: want.clone(),
            block: Some(blk),
        });
    }
    if !misses.is_empty() {
        let Some(path) = data_file.as_ref() else {
            return send_nack(app, cfg, stream, "blocks_range", seq_no).await;
        };
        if let Some(heights) = block_heights.as_ref() {
            for ix in misses.iter().copied() {
                let want = &block_hashes[ix];
                let Some(blk) = load_block_at_height(path, heights[ix])? else {
                    return send_nack(app, cfg, stream, "blocks_range", seq_no).await;
                };
                let got = hex::encode(hdr_hash(&blk.hdr));
                if got != *want {
                    return send_nack(app, cfg, stream, "blocks_hash", seq_no).await;
                }
                out[ix] = Some(SyncBlockWire {
                    height: blk.hdr.height,
                    hash: want.clone(),
                    block: Some(blk),
                });
            }
        } else {
            let miss_hashes: Vec<String> =
                misses.iter().map(|ix| block_hashes[*ix].clone()).collect();
            let scan = load_hash_scan_blocks(path, &miss_hashes)?;
            if scan.len() != miss_hashes.len() {
                return send_nack(app, cfg, stream, "blocks_range", seq_no).await;
            }
            for (scan_ix, blk_opt) in scan.into_iter().enumerate() {
                let row_ix = misses[scan_ix];
                let want = &block_hashes[row_ix];
                let Some(blk) = blk_opt else {
                    return send_nack(app, cfg, stream, "blocks_range", seq_no).await;
                };
                let got = hex::encode(hdr_hash(&blk.hdr));
                if got != *want {
                    return send_nack(app, cfg, stream, "blocks_hash", seq_no).await;
                }
                out[row_ix] = Some(SyncBlockWire {
                    height: blk.hdr.height,
                    hash: want.clone(),
                    block: Some(blk),
                });
            }
        }
    }
    let mut rows = Vec::with_capacity(out.len());
    for row in out.into_iter() {
        let Some(row) = row else {
            return send_nack(app, cfg, stream, "blocks_range", seq_no).await;
        };
        rows.push(SyncBlockWire {
            height: row.height,
            hash: row.hash,
            block: row.block,
        });
    }
    *seq_no = seq_no.saturating_add(1);
    let now_ms = current_time_ms().unwrap_or(0);
    write_wire_msg(
        stream,
        &PeerWireMsg::SyncBlocksBatch {
            hdr: SyncWireHdr {
                shard_id: app.identity.cluster_domain_hi,
                peer_session_id: app.identity.node_id.clone(),
                seq_no: *seq_no,
                timestamp_ms: now_ms,
            },
            blocks: rows,
        },
        cfg.heartbeat_timeout_ms,
    )
    .await?;
    let mut hs = app.handshake.write().await;
    hs.transport.snapshot.sync_blk_resp_total =
        hs.transport.snapshot.sync_blk_resp_total.saturating_add(1);
    Ok(())
}

fn reinqueue(st: &mut SyncPeerState, mut rows: VecDeque<(u64, String)>) {
    while let Some((height, hash)) = rows.pop_back() {
        if has_hash(&st.pend_blk, &hash) {
            continue;
        }
        st.pend_blk.push_front((height, hash));
        if st.pend_blk.len() > SYNC_PEND_CAP {
            st.pend_blk.pop_back();
        }
    }
}

fn cup_clear(st: &mut SyncPeerState) {
    st.cup_active = false;
    st.cup_epoch = 0;
    st.cup_from = 0;
    st.cup_to = 0;
    st.cup_next_h = 0;
    st.cup_next_ix = 0;
    st.cup_prev_hash.clear();
}

/// Abort epoch catch-up when peer announce lag (`head_h - local_h`) is inside short tail.
fn try_demote_cup_tail(hs: &mut HandshakeState, node_id: &str, tip_lag: u64) -> bool {
    if tip_lag >= SYNC_CUP_TAIL_MAX {
        return false;
    }
    let Some(st) = hs.sync_live.peers.get_mut(node_id) else {
        return false;
    };
    if !st.cup_active {
        return false;
    }
    hs.transport.snapshot.sync_cup_demote_tail =
        hs.transport.snapshot.sync_cup_demote_tail.saturating_add(1);
    cup_clear(st);
    st.cup_try = 0;
    st.cup_next_ms = 0;
    true
}

async fn cup_chunk_fail(app: &App, cfg: &TransportConfig, node_id: &str, reason: &str) {
    let mut hs = app.handshake.write().await;
    cup_fail(&mut hs, reason);
    let st = peer_sync(&mut hs, node_id);
    st.live_stall = st.live_stall.saturating_add(1);
    st.cup_try = st.cup_try.saturating_add(1);
    st.cup_next_ms = now_ms().saturating_add(cup_backoff_ms(cfg, st.cup_try));
    cup_clear(st);
    warn!(
        target: "pwmd::peer",
        "peer sync catchup fail node_id={} reason={} retry={} next_ms={}",
        node_id,
        reason,
        st.cup_try,
        st.cup_next_ms
    );
}

fn apply_blk(inn: &mut Inner, blk: &Block) -> Result<(), String> {
    let next_h = inn.chain.tip_h().saturating_add(1);
    if blk.hdr.height != next_h {
        return Err(format!(
            "height_mismatch want={} got={}",
            next_h, blk.hdr.height
        ));
    }
    let prev = inn.chain.tip_hash();
    if blk.hdr.prev_hash != prev {
        return Err("prev_hash_mismatch".to_string());
    }
    let vals_len = inn.chain.cfg.vals.set.len();
    if vals_len == 0 {
        return Err("empty_validators".to_string());
    }
    let want_idx = ((next_h.saturating_sub(1)) as usize % vals_len) as u32;
    if blk.hdr.prod_idx != want_idx {
        return Err(format!(
            "prod_idx_mismatch want={} got={}",
            want_idx, blk.hdr.prod_idx
        ));
    }
    let Some(prod) = inn.chain.cfg.vals.set.get(blk.hdr.prod_idx as usize) else {
        return Err("prod_idx_oor".to_string());
    };
    if !blk.hdr.verify_sig(&prod.pubkey) {
        return Err("bad_sig".to_string());
    }
    if txs_root(&blk.txs) != blk.hdr.tx_root {
        return Err("tx_root_mismatch".to_string());
    }
    let mut st = inn.chain.st.clone();
    for tx in blk.txs.iter() {
        st.apply_tx_with_ctx(tx, blk.hdr.height, blk.hdr.ts)
            .map_err(|e| format!("tx_invalid:{e}"))?;
    }
    let prod_acct = inn.chain.cfg.prod_acct(blk.hdr.prod_idx);
    if !st.accounts.contains_key(&prod_acct) {
        return Err("prod_acct_missing".to_string());
    }
    if inn.chain.cfg.is_legacy_policy() {
        st.reward_producer(&prod_acct, inn.chain.cfg.block_reward);
    } else {
        let season_ppm = inn.chain.cfg.season_ppm(blk.hdr.ts);
        st.reward_producer_v2(
            &prod_acct,
            inn.chain.cfg.block_reward,
            inn.chain.cfg.pwm_stake_min,
            season_ppm,
        );
    }
    if digest(&st) != blk.hdr.state_root {
        return Err("state_root_mismatch".to_string());
    }
    inn.chain.st = st;
    inn.chain.blocks.push_back(blk.clone());
    while inn.chain.blocks.len() > TAIL_BLOCK_CAP {
        inn.chain.blocks.pop_front();
    }
    inn.chain.set_canon_h(blk.hdr.height);
    for tx in blk.txs.iter() {
        inn.record_cross_shard_tx(tx, blk.hdr.height);
    }
    Ok(())
}

async fn apply_blk_batch(app: &App, blocks: &[Block]) -> Result<(), String> {
    let mut inn = app.inner.write().await;
    let mut bak_opt = Some(crate::api::common::take_bak(&inn));
    let tip_before = inn.chain.tip_h();
    for blk in blocks.iter() {
        if let Err(err) = apply_blk(&mut inn, blk) {
            if let Some(bak) = bak_opt.take() {
                crate::api::common::rollback_commit(&mut inn, bak);
            }
            return Err(err);
        }
    }
    let tip_h = inn.chain.tip_h();
    let range = tip_before.saturating_add(1)..=tip_h;
    let crossed_autosnap = range.clone().any(autosnap_hit);
    let crossed_standby = matches!(app.seal_role, SealRole::Standby)
        && range
            .clone()
            .any(|h| h == 1 || h % STANDBY_SYNC_FLUSH_IV == 0);
    let need_persist = crossed_autosnap || crossed_standby;
    if crossed_standby {
        info!(
            target: "pwmd::sync",
            "standby sync checkpoint range={}..{} flush_iv={}",
            tip_before.saturating_add(1),
            tip_h,
            STANDBY_SYNC_FLUSH_IV
        );
    }
    let save_result = app.autosnapshot_backend.as_ref().and_then(|backend| {
        need_persist.then(|| {
            (
                backend.init_state_path(),
                backend.save_seal_persist(&inn, SealPersistMode::Periodic),
            )
        })
    });
    let persist_bak = if save_result.is_some() {
        bak_opt.take()
    } else {
        None
    };
    drop(inn);
    periodic_snap_finish(app, tip_h, persist_bak, save_result).await;
    Ok(())
}

pub(super) async fn on_blk_batch(
    app: &App,
    cfg: &TransportConfig,
    stream: &mut tokio::net::TcpStream,
    node_id: &str,
    blocks: Vec<SyncBlockWire>,
    hdr_cap: u16,
    blk_cap: u16,
    seq_no: &mut u64,
) -> Result<(), String> {
    let hard = blk_cap.min(SYNC_BLK_REQ_CAP).max(1) as usize;
    if blocks.is_empty() || blocks.len() > hard {
        return Ok(());
    }
    {
        let hs = app.handshake.read().await;
        if hs
            .sync_live
            .peers
            .get(node_id)
            .map(|x| x.cup_active)
            .unwrap_or(false)
        {
            return Ok(());
        }
    }
    let expected = {
        let mut hs = app.handshake.write().await;
        hs.transport.snapshot.sync_blk_resp_total =
            hs.transport.snapshot.sync_blk_resp_total.saturating_add(1);
        let st = peer_sync(&mut hs, node_id);
        st.in_blk = st.in_blk.saturating_sub(1);
        std::mem::take(&mut st.wait_blk)
    };
    if expected.is_empty() {
        return Ok(());
    }
    let mut rest = expected;
    let mut apply_rows: Vec<Block> = Vec::new();
    for row in blocks.into_iter() {
        let Some((want_h, want_hash)) = rest.pop_front() else {
            break;
        };
        if row.hash != want_hash || row.height != want_h {
            let mut hs = app.handshake.write().await;
            hs.transport.snapshot.sync_fork_conflict_total = hs
                .transport
                .snapshot
                .sync_fork_conflict_total
                .saturating_add(1);
            let st = peer_sync(&mut hs, node_id);
            st.live_stall = st.live_stall.saturating_add(1);
            reinqueue(st, rest);
            st.pend_blk.push_front((want_h, want_hash));
            return Ok(());
        }
        let Some(blk) = row.block else {
            let mut hs = app.handshake.write().await;
            hs.transport.snapshot.sync_apply_fail_total = hs
                .transport
                .snapshot
                .sync_apply_fail_total
                .saturating_add(1);
            let st = peer_sync(&mut hs, node_id);
            st.live_stall = st.live_stall.saturating_add(1);
            reinqueue(st, rest);
            st.pend_blk.push_front((want_h, want_hash));
            return Ok(());
        };
        let got_hash = hex::encode(hdr_hash(&blk.hdr));
        if got_hash != row.hash || blk.hdr.height != row.height || blk.hdr.height != want_h {
            let mut hs = app.handshake.write().await;
            hs.transport.snapshot.sync_fork_conflict_total = hs
                .transport
                .snapshot
                .sync_fork_conflict_total
                .saturating_add(1);
            let st = peer_sync(&mut hs, node_id);
            st.live_stall = st.live_stall.saturating_add(1);
            reinqueue(st, rest);
            st.pend_blk.push_front((want_h, want_hash));
            return Ok(());
        }
        apply_rows.push(blk);
    }
    match apply_blk_batch(app, &apply_rows).await {
        Ok(()) => {
            let tip_h = {
                let mut hs = app.handshake.write().await;
                hs.transport.snapshot.sync_apply_ok_total = hs
                    .transport
                    .snapshot
                    .sync_apply_ok_total
                    .saturating_add(apply_rows.len() as u64);
                let st = peer_sync(&mut hs, node_id);
                st.live_stall = 0;
                reinqueue(st, rest);
                st.tip_h
            };
            let local_h = {
                let g = app.inner.read().await;
                g.chain.tip_h()
            };
            let persisted_h = app.last_snapshot_height.load(Ordering::Acquire);
            maybe_log_sync_prog(app, node_id, local_h, tip_h, persisted_h).await;
            info!(
                target: "pwmd::peer",
                "peer sync apply ok node_id={} blocks={}",
                node_id,
                apply_rows.len()
            );
            live_tail_pull_hdr(app, cfg, stream, node_id, hdr_cap, seq_no, local_h, tip_h).await?;
        }
        Err(err) => {
            warn!(
                target: "pwmd::peer",
                "peer sync apply failed node_id={} reason={}",
                node_id,
                err
            );
            let mut hs = app.handshake.write().await;
            hs.transport.snapshot.sync_apply_fail_total = hs
                .transport
                .snapshot
                .sync_apply_fail_total
                .saturating_add(1);
            let st = peer_sync(&mut hs, node_id);
            st.live_stall = st.live_stall.saturating_add(1);
            reinqueue(st, rest);
        }
    }
    ask_blk(app, cfg, stream, node_id, blk_cap, seq_no).await
}

pub(super) async fn on_cup_req(
    app: &App,
    cfg: &TransportConfig,
    stream: &mut tokio::net::TcpStream,
    start_h: u64,
    end_h: u64,
    epoch_id: u64,
    seq_no: &mut u64,
) -> Result<(), String> {
    if start_h == 0
        || end_h < start_h
        || end_h.saturating_sub(start_h).saturating_add(1) > SYNC_CUP_WIN_CAP
    {
        return send_nack(app, cfg, stream, "catchup_range", seq_no).await;
    }
    if epoch_idx(start_h)? != epoch_id || epoch_idx(end_h)? != epoch_id {
        return send_nack(app, cfg, stream, "catchup_epoch", seq_no).await;
    }
    let mut rows = {
        let g = app.inner.read().await;
        let mut out = Vec::new();
        for blk in g
            .chain
            .blocks
            .iter()
            .filter(|x| x.hdr.height >= start_h && x.hdr.height <= end_h)
        {
            out.push(blk.clone());
        }
        out
    };
    let need_disk = rows.is_empty()
        || rows.first().map(|x| x.hdr.height) != Some(start_h)
        || rows.last().map(|x| x.hdr.height) != Some(end_h);
    if need_disk {
        if let Some(path) = app.data_file.as_ref() {
            let lim = usize::try_from(end_h.saturating_sub(start_h).saturating_add(1))
                .map_err(|_| "catchup_range_overflow".to_string())?;
            if let Ok(disk_rows) = load_consecutive_blocks_from_epochs(path, start_h, lim) {
                if disk_rows.first().map(|x| x.hdr.height) == Some(start_h)
                    && disk_rows.last().map(|x| x.hdr.height) == Some(end_h)
                {
                    rows = disk_rows;
                }
            }
        }
    }
    if rows.is_empty()
        || rows.first().map(|x| x.hdr.height) != Some(start_h)
        || rows.last().map(|x| x.hdr.height) != Some(end_h)
    {
        return send_nack(app, cfg, stream, "catchup_gap", seq_no).await;
    }
    let chunk_sz = usize::from(SYNC_BLK_REQ_CAP.min(SYNC_HDR_REQ_CAP)).min(SYNC_CUP_CHUNK_CAP);
    let mut idx = 0u32;
    for chunk_rows in rows.chunks(chunk_sz.max(1)) {
        let mut hdrs = Vec::with_capacity(chunk_rows.len());
        let mut blks = Vec::with_capacity(chunk_rows.len());
        for blk in chunk_rows.iter() {
            hdrs.push(SyncHeaderWire {
                height: blk.hdr.height,
                hash: hex::encode(hdr_hash(&blk.hdr)),
                prev_hash: hex::encode(blk.hdr.prev_hash),
            });
            blks.push(SyncBlockWire {
                height: blk.hdr.height,
                hash: hex::encode(hdr_hash(&blk.hdr)),
                block: Some(blk.clone()),
            });
        }
        let first_prev = hdrs
            .first()
            .map(|x| x.prev_hash.clone())
            .unwrap_or_default();
        let last_hash = hdrs.last().map(|x| x.hash.clone()).unwrap_or_default();
        *seq_no = seq_no.saturating_add(1);
        write_wire_msg(
            stream,
            &PeerWireMsg::SyncCatchupChunk {
                hdr: SyncWireHdr {
                    shard_id: app.identity.cluster_domain_hi,
                    peer_session_id: app.identity.node_id.clone(),
                    seq_no: *seq_no,
                    timestamp_ms: now_ms(),
                },
                chunk: SyncCatchupChunkWire {
                    epoch_id,
                    chunk_index: idx,
                    first_prev_hash: first_prev,
                    last_hash,
                    headers: hdrs,
                    blocks: blks,
                },
            },
            cfg.heartbeat_timeout_ms,
        )
        .await?;
        idx = idx.saturating_add(1);
    }
    let last_hash = rows
        .last()
        .map(|x| hex::encode(hdr_hash(&x.hdr)))
        .unwrap_or_default();
    *seq_no = seq_no.saturating_add(1);
    write_wire_msg(
        stream,
        &PeerWireMsg::SyncCatchupDone {
            hdr: SyncWireHdr {
                shard_id: app.identity.cluster_domain_hi,
                peer_session_id: app.identity.node_id.clone(),
                seq_no: *seq_no,
                timestamp_ms: now_ms(),
            },
            epoch_id,
            last_height: end_h,
            last_hash,
        },
        cfg.heartbeat_timeout_ms,
    )
    .await
}

pub(super) async fn on_cup_chunk(
    app: &App,
    cfg: &TransportConfig,
    node_id: &str,
    chunk: SyncCatchupChunkWire,
) {
    if chunk.headers.is_empty()
        || chunk.headers.len() > SYNC_CUP_CHUNK_CAP
        || chunk.headers.len() != chunk.blocks.len()
    {
        cup_chunk_fail(app, cfg, node_id, "chunk_bounds").await;
        return;
    }
    let (mut next_h, mut next_ix, mut prev_hash, from_h, to_h, epoch_id, active) = {
        let hs = app.handshake.read().await;
        let Some(st) = hs.sync_live.peers.get(node_id) else {
            return;
        };
        (
            st.cup_next_h,
            st.cup_next_ix,
            st.cup_prev_hash.clone(),
            st.cup_from,
            st.cup_to,
            st.cup_epoch,
            st.cup_active,
        )
    };
    if !active {
        return;
    }
    if chunk.epoch_id != epoch_id
        || chunk.chunk_index != next_ix
        || chunk.first_prev_hash != prev_hash
    {
        cup_chunk_fail(app, cfg, node_id, "chunk_order").await;
        return;
    }
    let mut apply_rows = Vec::with_capacity(chunk.blocks.len());
    for (hdr, row) in chunk.headers.iter().zip(chunk.blocks.iter()) {
        if hdr.height != next_h
            || hdr.prev_hash != prev_hash
            || row.height != hdr.height
            || row.hash != hdr.hash
        {
            cup_chunk_fail(app, cfg, node_id, "chunk_link").await;
            return;
        }
        let Some(blk) = row.block.clone() else {
            cup_chunk_fail(app, cfg, node_id, "chunk_empty").await;
            return;
        };
        let got_hash = hex::encode(hdr_hash(&blk.hdr));
        if got_hash != hdr.hash {
            cup_chunk_fail(app, cfg, node_id, "chunk_hash").await;
            return;
        }
        if hdr.height < from_h || hdr.height > to_h {
            cup_chunk_fail(app, cfg, node_id, "chunk_range").await;
            return;
        }
        prev_hash = hdr.hash.clone();
        next_h = next_h.saturating_add(1);
        apply_rows.push(blk);
    }
    if chunk.last_hash != prev_hash {
        cup_chunk_fail(app, cfg, node_id, "chunk_tail").await;
        return;
    }
    if apply_blk_batch(app, &apply_rows).await.is_err() {
        cup_chunk_fail(app, cfg, node_id, "chunk_apply").await;
        return;
    }
    let (local_h, tip_h) = {
        let mut hs = app.handshake.write().await;
        hs.transport.snapshot.sync_cup_chunk_total =
            hs.transport.snapshot.sync_cup_chunk_total.saturating_add(1);
        let st = peer_sync(&mut hs, node_id);
        st.cup_prev_hash = prev_hash;
        st.cup_next_h = next_h;
        next_ix = next_ix.saturating_add(1);
        st.cup_next_ix = next_ix;
        st.live_stall = 0;
        (
            st.cup_next_h.saturating_sub(1),
            st.tip_h.max(st.cup_target_h),
        )
    };
    let persisted_h = app.last_snapshot_height.load(Ordering::Acquire);
    maybe_log_sync_prog(app, node_id, local_h, tip_h, persisted_h).await;
    info!(
        target: "pwmd::peer",
        "peer sync catchup progress node_id={} epoch_id={} next_height={}",
        node_id,
        epoch_id,
        local_h.saturating_add(1)
    );
}

pub(super) async fn on_cup_done(
    app: &App,
    cfg: &TransportConfig,
    stream: &mut tokio::net::TcpStream,
    node_id: &str,
    epoch_id: u64,
    last_h: u64,
    last_hash: &str,
    hdr_cap: u16,
    seq_no: &mut u64,
) -> Result<(), String> {
    let mut need_live = false;
    let mut next_live_h = 0u64;
    let tip_h;
    {
        let mut hs = app.handshake.write().await;
        let mut done_ok = false;
        let mut done_bad = false;
        let mut out_tip = 0u64;
        {
            let st = peer_sync(&mut hs, node_id);
            if !st.cup_active || st.cup_epoch != epoch_id {
                return Ok(());
            }
            if st.cup_next_h.saturating_sub(1) != last_h || st.cup_prev_hash != last_hash {
                st.cup_try = st.cup_try.saturating_add(1);
                st.cup_next_ms = now_ms().saturating_add(cup_backoff_ms(cfg, st.cup_try));
                cup_clear(st);
                done_bad = true;
            } else {
                st.cup_try = 0;
                st.live_stall = 0;
                out_tip = st.tip_h;
                cup_clear(st);
                done_ok = true;
            }
        }
        if done_bad {
            cup_fail(&mut hs, "done_mismatch");
            warn!(
                target: "pwmd::peer",
                "peer sync catchup fail node_id={} reason=done_mismatch epoch_id={} last_height={}",
                node_id,
                epoch_id,
                last_h
            );
            return Ok(());
        }
        if done_ok {
            hs.transport.snapshot.sync_cup_done_total =
                hs.transport.snapshot.sync_cup_done_total.saturating_add(1);
        }
        tip_h = out_tip;
    }
    let local_h = {
        let g = app.inner.read().await;
        g.chain.tip_h()
    };
    if tip_h > local_h {
        need_live = true;
        next_live_h = local_h.saturating_add(1);
    }
    let persisted_h = app.last_snapshot_height.load(Ordering::Acquire);
    maybe_log_sync_prog(app, node_id, local_h, tip_h.max(last_h), persisted_h).await;
    info!(
        target: "pwmd::peer",
        "peer sync catchup finish node_id={} epoch_id={} last_height={}",
        node_id,
        epoch_id,
        last_h
    );
    if need_live {
        let req_lim = hdr_cap.min(SYNC_HDR_REQ_CAP).max(1);
        ask_hdr(app, cfg, stream, node_id, next_live_h, req_lim, seq_no).await?;
    }
    Ok(())
}

pub(super) async fn on_nack(app: &App, cfg: &TransportConfig, node_id: &str, reason_code: &str) {
    let mut hs = app.handshake.write().await;
    let mut cup_drop = false;
    let mut cup_retry = 0u8;
    let mut cup_next_ms = 0u64;
    let st = peer_sync(&mut hs, node_id);
    st.in_hdr = st.in_hdr.saturating_sub(1);
    st.in_blk = st.in_blk.saturating_sub(1);
    st.live_stall = st.live_stall.saturating_add(1);
    st.wait_hdr_from = None;
    let wait = std::mem::take(&mut st.wait_blk);
    reinqueue(st, wait);
    if st.cup_active {
        let is_epoch_nack = reason_code == "catchup_epoch";
        if is_epoch_nack {
            // Retry immediately with an epoch-local CUP window on the next tip update.
            st.cup_try = 0;
            st.cup_next_ms = 0;
        } else {
            st.cup_try = st.cup_try.saturating_add(1);
            st.cup_next_ms = now_ms().saturating_add(cup_backoff_ms(cfg, st.cup_try));
        }
        cup_retry = st.cup_try;
        cup_next_ms = st.cup_next_ms;
        cup_clear(st);
        cup_drop = true;
    }
    if cup_drop {
        if reason_code == "catchup_epoch" {
            cup_fail(&mut hs, "nack:catchup_epoch");
        } else {
            cup_fail(&mut hs, "nack");
        }
        warn!(
            target: "pwmd::peer",
            "peer sync catchup aborted by nack node_id={} reason={} retry={} next_ms={}",
            node_id,
            reason_code,
            cup_retry,
            cup_next_ms
        );
    }
    info!(
        target: "pwmd::peer",
        "peer sync nack node_id={} reason={}",
        node_id,
        reason_code
    );
}

// Apply `apply_blk_batch` + JsonFile autosnapshot at mod-100 tip (sync checkpoint path).
#[cfg(test)]
mod tests {
    use super::{
        apply_blk_batch, maybe_start_cup, on_hdr_req, sync_prog_snap, sync_prog_tail_quiet,
        sync_prog_tick, try_demote_cup_tail, SyncProgSnap,
    };
    use crate::bootstrap::{app_from_dev_net, app_from_genesis_id};
    use crate::config::GenesisSource;
    use crate::handshake::{HandshakeValidationCtx, SealRole};
    use crate::identity::DevLane;
    use crate::snapshot::epoch::{manifest_file_path, EpochManifest};
    use crate::snapshot::incremental::append_tip_block;
    use crate::transport::handshake_state::{HandshakeState, SyncPeerState};
    use crate::transport::peer_session::wire::{read_wire_msg, PeerWireMsg};
    use pwm_core::chain::{Chain, SealTimeMode};
    use pwm_core::{dev_net, TAIL_BLOCK_CAP};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn sync_prog_snap_goal_rules() {
        assert_eq!(sync_prog_snap(0, 0, 0), None);
        assert_eq!(
            sync_prog_snap(0, 200, 0),
            Some(SyncProgSnap {
                goal: 200,
                pct: 0,
                rem: 200
            })
        );
        assert_eq!(
            sync_prog_snap(199, 200, 0),
            Some(SyncProgSnap {
                goal: 200,
                pct: 99,
                rem: 1
            })
        );
        assert_eq!(
            sync_prog_snap(250, 200, 0),
            Some(SyncProgSnap {
                goal: 200,
                pct: 100,
                rem: 0
            })
        );
        assert_eq!(
            sync_prog_snap(10, 0, 0),
            Some(SyncProgSnap {
                goal: 10,
                pct: 100,
                rem: 0
            })
        );
    }

    #[test]
    fn short_tail_quiet_policy() {
        assert!(sync_prog_tail_quiet(false, 101, 100));
        assert!(!sync_prog_tail_quiet(true, 101, 100));
        // tip_lag 150 ≥ 32 → not quiet even though `rem` in snap would be large
        assert!(!sync_prog_tail_quiet(false, 200, 50));
        // Small lag with several blocks left: still "short tail" for Standby log suppression
        assert!(sync_prog_tail_quiet(false, 120, 100));
        assert!(!sync_prog_tail_quiet(false, 0, 100));
    }

    #[test]
    fn sync_prog_tick_throttle_done() {
        let mut st = SyncPeerState::default();
        assert_eq!(
            sync_prog_tick(&mut st, 1_000, 90, 100, 0),
            Some(SyncProgSnap {
                goal: 100,
                pct: 90,
                rem: 10
            })
        );
        assert_eq!(sync_prog_tick(&mut st, 2_000, 91, 100, 0), None);
        assert_eq!(sync_prog_tick(&mut st, 8_500, 91, 100, 0), None);
        assert_eq!(
            sync_prog_tick(&mut st, 62_000, 91, 100, 0),
            Some(SyncProgSnap {
                goal: 100,
                pct: 91,
                rem: 9
            })
        );
        assert_eq!(
            sync_prog_tick(&mut st, 63_000, 100, 100, 0),
            Some(SyncProgSnap {
                goal: 100,
                pct: 100,
                rem: 0
            })
        );
        assert_eq!(sync_prog_tick(&mut st, 63_500, 100, 100, 0), None);
        assert_eq!(sync_prog_tick(&mut st, 10_000, 0, 0, 0), None);
    }

    /// After a finished sync (`sync_log_done`), a higher peer tip should log again once
    /// `SYNC_PROG_TAIL_MS` elapsed in the short-tail zone (`quiet_goal_bump`) or
    /// `SYNC_PROG_MIN_MS` outside it. Same numbers as the tail of `sync_prog_tick_throttle_done`,
    /// split for grep/regression clarity.
    #[test]
    fn sync_prog_tick_lag_resume() {
        let mut st = SyncPeerState::default();
        assert_eq!(
            sync_prog_tick(&mut st, 1_000, 90, 100, 0),
            Some(SyncProgSnap {
                goal: 100,
                pct: 90,
                rem: 10
            })
        );
        assert_eq!(
            sync_prog_tick(&mut st, 9_000, 100, 100, 0),
            Some(SyncProgSnap {
                goal: 100,
                pct: 100,
                rem: 0
            })
        );
        assert_eq!(sync_prog_tick(&mut st, 9_500, 100, 100, 0), None);
        assert_eq!(
            sync_prog_tick(&mut st, 69_000, 100, 105, 0),
            Some(SyncProgSnap {
                goal: 105,
                pct: 95,
                rem: 5
            })
        );
    }

    /// Steady tip: do not print another `Sync progress 100%` until peer goal advances.
    #[test]
    fn sync_prog_pct100_dedup() {
        let mut st = SyncPeerState::default();
        assert_eq!(
            sync_prog_tick(&mut st, 1_000, 99, 100, 0),
            Some(SyncProgSnap {
                goal: 100,
                pct: 99,
                rem: 1
            })
        );
        assert_eq!(
            sync_prog_tick(&mut st, 2_000, 100, 100, 0),
            Some(SyncProgSnap {
                goal: 100,
                pct: 100,
                rem: 0
            })
        );
        assert_eq!(sync_prog_tick(&mut st, 2_050, 100, 100, 0), None);
        assert_eq!(sync_prog_tick(&mut st, 9_000, 100, 100, 0), None);
        assert_eq!(
            sync_prog_tick(&mut st, 62_000, 100, 101, 0),
            Some(SyncProgSnap {
                goal: 101,
                pct: 99,
                rem: 1
            })
        );
        assert_eq!(
            sync_prog_tick(&mut st, 62_001, 101, 101, 0),
            Some(SyncProgSnap {
                goal: 101,
                pct: 100,
                rem: 0
            })
        );
        assert_eq!(sync_prog_tick(&mut st, 62_050, 101, 101, 0), None);
    }

    #[test]
    fn cup_tail_demote_height_delta() {
        let validation_ctx = HandshakeValidationCtx {
            expected_network_id: "devnet".to_string(),
            expected_genesis_hash: None,
            skew_window_ms: 30_000,
        };
        let mut hs = HandshakeState::new(validation_ctx, 1);
        hs.sync_live.peers.insert(
            "peer-a".to_string(),
            SyncPeerState {
                cup_active: true,
                cup_try: 2,
                cup_next_ms: 9_999,
                ..Default::default()
            },
        );
        assert!(!try_demote_cup_tail(
            &mut hs,
            "peer-a",
            super::SYNC_CUP_TAIL_MAX
        ));
        assert!(try_demote_cup_tail(
            &mut hs,
            "peer-a",
            super::SYNC_CUP_TAIL_MAX - 1
        ));
        let st = hs.sync_live.peers.get("peer-a").expect("peer");
        assert!(!st.cup_active);
        assert_eq!(st.cup_try, 0);
        assert_eq!(st.cup_next_ms, 0);
        assert_eq!(hs.transport.snapshot.sync_cup_demote_tail, 1);
        assert!(!try_demote_cup_tail(&mut hs, "peer-a", 0));
    }

    #[tokio::test]
    async fn cup_req_stays_in_epoch() {
        let app = app_from_dev_net();
        {
            let mut g = app.inner.write().await;
            for _ in 0..544 {
                g.chain.seal(vec![]).expect("seal");
            }
            assert_eq!(g.chain.tip_h(), 544);
        }
        let cfg = app.transport_config.read().await.clone();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind");
        let addr = listener.local_addr().expect("local_addr");
        let cli_task = tokio::spawn(async move { tokio::net::TcpStream::connect(addr).await });
        let (mut srv, _) = listener.accept().await.expect("accept");
        let mut cli = cli_task.await.expect("join").expect("connect");

        let mut seq_no = 0u64;
        let started = maybe_start_cup(&app, &cfg, &mut srv, "peer-cup", 4_640, true, &mut seq_no)
            .await
            .expect("cup start");
        assert!(started, "expected catchup mode");

        let msg = read_wire_msg(&mut cli, cfg.heartbeat_timeout_ms)
            .await
            .expect("read msg");
        match msg {
            PeerWireMsg::SyncCatchupReq {
                start_height,
                end_height,
                epoch_id,
                ..
            } => {
                assert_eq!(start_height, 545);
                assert_eq!(end_height, 1_000);
                assert_eq!(
                    epoch_id,
                    crate::snapshot::epoch::epoch_idx(545).expect("epoch idx")
                );
            }
            other => panic!("unexpected wire frame: {other:?}"),
        }
    }

    #[tokio::test]
    async fn batch_cross_ckpt_writes_snap() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let snap_dir = std::env::temp_dir().join(format!("pwmd-syncautosnap-{suffix}"));
        let _ = std::fs::remove_dir_all(&snap_dir);
        std::fs::create_dir_all(&snap_dir).expect("mk dir");
        let snapshot_path = snap_dir.join("pwm-data.json");

        let (cfg, sks) = dev_net();
        let mut producer = Chain::boot(cfg, sks);
        producer.set_seal_time_mode(SealTimeMode::DeterministicHeight);
        let mut pre_blocks = Vec::with_capacity(96);
        let mut cross_blocks = Vec::with_capacity(9);
        for _ in 0..105 {
            producer.seal(vec![]).expect("producer seal");
            let blk = producer.blocks.back().expect("blk").clone();
            if blk.hdr.height <= 96 {
                pre_blocks.push(blk);
            } else {
                cross_blocks.push(blk);
            }
        }

        let app = app_from_genesis_id(
            &GenesisSource::DevNet,
            DevLane::Lane0,
            Some(snapshot_path.clone()),
            None,
        )
        .expect("app boot");

        apply_blk_batch(&app, &pre_blocks)
            .await
            .expect("prime batch must succeed");
        apply_blk_batch(&app, &cross_blocks)
            .await
            .expect("apply batch must succeed");

        let manifest_path = manifest_file_path(&snapshot_path);
        assert!(
            manifest_path.exists(),
            "expected epoch manifest after sync_apply autosnapshot"
        );
        let raw = std::fs::read_to_string(&manifest_path).expect("read manifest");
        let man: EpochManifest = serde_json::from_str(&raw).expect("parse manifest");
        assert_eq!(man.canonical_h, 105, "manifest tip should match batch end");

        let _ = std::fs::remove_dir_all(&snap_dir);
    }

    #[tokio::test]
    async fn standby_batch_cross_iv_writes() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let snap_dir = std::env::temp_dir().join(format!("pwmd-syncstandby-{suffix}"));
        let _ = std::fs::remove_dir_all(&snap_dir);
        std::fs::create_dir_all(&snap_dir).expect("mk dir");
        let snapshot_path = snap_dir.join("pwm-data.json");

        let (cfg, sks) = dev_net();
        let mut producer = Chain::boot(cfg, sks);
        producer.set_seal_time_mode(SealTimeMode::DeterministicHeight);
        let mut pre_blocks = Vec::with_capacity(95);
        let mut sync_blocks = Vec::with_capacity(6);
        for _ in 0..101 {
            producer.seal(vec![]).expect("producer seal");
            let blk = producer.blocks.back().expect("blk").clone();
            if blk.hdr.height <= 95 {
                pre_blocks.push(blk);
            } else {
                sync_blocks.push(blk);
            }
        }

        let mut app = app_from_genesis_id(
            &GenesisSource::DevNet,
            DevLane::Lane0,
            Some(snapshot_path.clone()),
            None,
        )
        .expect("app boot");

        apply_blk_batch(&app, &pre_blocks)
            .await
            .expect("prime batch must succeed");
        app.seal_role = SealRole::Standby;
        apply_blk_batch(&app, &sync_blocks)
            .await
            .expect("standby batch must persist");

        let manifest_path = manifest_file_path(&snapshot_path);
        assert!(
            manifest_path.exists(),
            "expected standby sync checkpoint at STANDBY_SYNC_FLUSH_IV boundary (height 100)"
        );
        let raw = std::fs::read_to_string(&manifest_path).expect("read manifest");
        let man: EpochManifest = serde_json::from_str(&raw).expect("parse manifest");
        assert_eq!(
            man.canonical_h, 101,
            "manifest tip should match standby batch end"
        );

        let _ = std::fs::remove_dir_all(&snap_dir);
    }

    #[tokio::test]
    async fn hdr_req_disk_below_tail() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let snap_dir = std::env::temp_dir().join(format!("pwmd-sync-hdr-tail-{suffix}"));
        let _ = std::fs::remove_dir_all(&snap_dir);
        std::fs::create_dir_all(&snap_dir).expect("mkdir");
        let data_path = snap_dir.join("pwm-data.json");

        let mut app = app_from_dev_net();
        app.data_file = Some(data_path.clone());
        {
            let mut g = app.inner.write().await;
            for _ in 0..(TAIL_BLOCK_CAP as u64 + 12) {
                g.chain.seal(vec![]).expect("seal");
                append_tip_block(&data_path, &g).expect("append");
            }
            let mem_first_h = g.chain.blocks.front().expect("mem head").hdr.height;
            assert!(mem_first_h > 1, "memory tail must be above genesis");
        }
        let cfg = app.transport_config.read().await.clone();

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind");
        let addr = listener.local_addr().expect("local_addr");
        let cli_task = tokio::spawn(async move { tokio::net::TcpStream::connect(addr).await });
        let (mut srv, _) = listener.accept().await.expect("accept");
        let mut cli = cli_task.await.expect("join").expect("connect");

        let mut seq_no = 0u64;
        on_hdr_req(&app, &cfg, &mut srv, 1, 8, 32, &mut seq_no)
            .await
            .expect("hdr req");

        let msg = read_wire_msg(&mut cli, cfg.heartbeat_timeout_ms)
            .await
            .expect("read msg");
        match msg {
            PeerWireMsg::SyncHeadersBatch { headers, .. } => {
                assert_eq!(headers.first().map(|x| x.height), Some(1));
                assert!(headers.len() >= 2);
            }
            other => panic!("unexpected wire frame: {other:?}"),
        }

        let _ = std::fs::remove_dir_all(&snap_dir);
    }
}
