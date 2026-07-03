//! Block ticks, mempool drains, autosnapshot triggers, and federation sweep hooks.
//! `spawn_snapshot_loader` logs `Instant`-based stages on target `pwmd::startup::snapshot`.

use crate::api::common::{rollback_commit, take_bak};
use crate::block_timing;
use crate::block_writer::BlockWriter;
use crate::bootstrap::app_from_genesis_id;
use crate::config::PwmdConfig;
use crate::debug_dump::{align_mid_on, mid_wait_ms};
use crate::handshake::{ClusterRole, DeploymentProfile, SealRole};
use crate::lease::{step_lease, LeaseCfg, LeaseEvent, LeaseState};
use crate::ledger::{summary_log_line, SUMMARY_BLOCK_INTERVAL};
use crate::perfmon;
use crate::pipeline::{counters, TxEvent};
use crate::runtime_shard_label;
use crate::snapshot::{
    BlocksStored, SealPersistMode, SnapIoTiming, SnapshotBackend, SnapshotLoadOpts,
    SNAP_STARTUP_TARGET,
};
use crate::state::{App, InitPhase, InitState};
use crate::storage_namespace;
use crate::transport::record_cluster_prop_tick;
use crate::RuntimeIdentityMode;
use crate::{
    cors_for_listen, digest, federation::spawn_federation_sweep_loop, router,
    spawn_peer_listener_loop, spawn_stateful_transport_loop, spawn_transport_loop,
};
use pwm_core::absorb_blocks_tail;
use pwm_core::block::Block;
use pwm_core::chain::{pick_prod_idx, roll_epoch_if_needed};
use pwm_core::state::State;
use pwm_core::tx::{SignedTx, TxBody};
use pwm_core::{SealEntry, SealTimeMode};
use std::collections::HashSet;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tracing::{debug, debug_span, error, info, warn};

pub const AUTOSNAPSHOT_BLOCK_INTERVAL: u64 = crate::snapshot::epoch::SNAP_CHK_BLK_IV;
/// Standby sync checkpoint interval (height 1 and then every N blocks).
pub const STANDBY_SYNC_FLUSH_IV: u64 = 100;
const SEAL_HOUR_MS: u64 = 3_600_000;
const CLUSTER_ATTEST_JITTER_MS: u64 = 500;
const SEAL_DRIFT_WINDOW_BLOCKS: u64 = 100;
const SEAL_DRIFT_STEP_PPM: u64 = 10_000;
const PPM_DENOM: u64 = 1_000_000;
/// ±1% envelope around nominal seal cadence (owner invariant).
const SEAL_ENVELOPE_LO_NUM: u64 = 990;
const SEAL_ENVELOPE_HI_NUM: u64 = 1010;
const SEAL_ENVELOPE_DENOM: u64 = 1000;
/// Deadband on |actual-expected|/expected below which drift adjust is skipped (0.1%).
const SEAL_DRIFT_DEADBAND_PPM: u64 = 1_000;
/// Wall-clock window for proposer seal-loop suppression aggregate (owner observability).
const SEAL_SUPPRESS_WINDOW_SEC: u64 = 100;
const PREP_SUMMARY_IV_SEC: u64 = 30;
/// Suppression percent threshold above which the 100s summary emits at ERROR level.
const SEAL_SUPPRESS_ALERT_PCT: f64 = 1.0;
/// Wall-clock poll cadence for the proposer deadline scheduler (variant C).
/// Under load gates re-check every 10 ms; under no load it bounds idle sleep.
const SEAL_POLL_INTERVAL_MS: u64 = 10;
/// Minimum interval between identical CAS-miss warnings in lease gate path.
const LEASE_REJECT_WARN_MS: u64 = 5_000;
/// Idle pause when no attester peer is connected and quorum is impossible.
/// Slow enough to keep CPU/log quiet, fast enough to react when attester joins.
const SEAL_WAIT_PEER_MS: u64 = 500;
/// Liveness window for an attester peer record: `now - last_seen_ms` must be ≤ this.
const ATTESTER_LIVE_WINDOW_MS: u64 = 5_000;
/// Bounded proposer resend attempts after `got=0` quorum timeout.
const CLUSTER_PROP_RETRY_CAP: u8 = 2;
const PROD_PICK_EMPTY_ERR: &str = "no active validators for current epoch";

pub(crate) fn autosnap_hit(h: u64) -> bool {
    h > 0 && h % AUTOSNAPSHOT_BLOCK_INTERVAL == 0
}

pub(crate) fn cluster_pending_summary_hit(h: u64) -> bool {
    h > 0 && h % 10 == 0
}

pub(crate) fn lease_renew_log_hit(last_tip: &std::sync::atomic::AtomicU64, h: u64) -> bool {
    if h != 1 && h % 10 != 0 {
        return false;
    }
    loop {
        let prev = last_tip.load(Ordering::Acquire);
        if prev == h {
            return false;
        }
        if last_tip
            .compare_exchange(prev, h, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
        {
            return true;
        }
    }
}

#[derive(Default)]
struct LeaseRejectWarnDedup {
    last_reason: Option<String>,
    last_warn_ms: u64,
    suppressed: u64,
}

fn lease_reject_warn_dedup() -> &'static Mutex<LeaseRejectWarnDedup> {
    static DEDUP: OnceLock<Mutex<LeaseRejectWarnDedup>> = OnceLock::new();
    DEDUP.get_or_init(|| Mutex::new(LeaseRejectWarnDedup::default()))
}

fn lease_reject_warn_suppressed(now_ms: u64, reason: &str) -> Option<u64> {
    let Ok(mut st) = lease_reject_warn_dedup().lock() else {
        return Some(0);
    };
    if st.last_reason.as_deref() != Some(reason) {
        st.last_reason = Some(reason.to_string());
        st.last_warn_ms = now_ms;
        st.suppressed = 0;
        return Some(0);
    }
    if now_ms.saturating_sub(st.last_warn_ms) >= LEASE_REJECT_WARN_MS {
        let suppressed = st.suppressed;
        st.last_warn_ms = now_ms;
        st.suppressed = 0;
        return Some(suppressed);
    }
    st.suppressed = st.suppressed.saturating_add(1);
    None
}

pub(crate) fn seal_interval_ms(blocks_per_hour: u64) -> Result<u64, String> {
    if blocks_per_hour == 0 {
        return Err("genesis blocks_per_hour must be greater than zero".to_string());
    }
    Ok((SEAL_HOUR_MS / blocks_per_hour).max(1))
}

pub(crate) fn cluster_timing_ms(seal_ms: u64) -> u64 {
    // RFC16 timing is genesis-derived: quorum timeout gets one nominal seal tick plus slack.
    let seal_ms = seal_ms.max(1);
    seal_ms
        .saturating_mul(2)
        .max(seal_ms.saturating_add(CLUSTER_ATTEST_JITTER_MS))
}

pub(crate) fn cluster_prop_ms(seal_ms: u64, heartbeat_ms: u64) -> u64 {
    heartbeat_ms.max(1).min(seal_ms.max(1))
}

fn apply_cluster_timing(
    cluster: &mut crate::ClusterCfg,
    transport: &mut crate::TransportConfig,
    bph: u64,
) -> Result<u64, String> {
    let seal_ms = seal_interval_ms(bph)?;
    if cluster.enabled {
        cluster.attest_timeout_ms = cluster_timing_ms(seal_ms);
        // Heartbeat must not exceed seal cadence on any cluster role: attester
        // ACKs gate proposer's quorum, so a 1500ms attester heartbeat starves a
        // 1000ms proposer seal loop (~50% suppression). Cap heartbeat = min of
        // configured value and seal_ms for proposer, attester, and any future
        // committer role. RFC16 §timing.
        transport.heartbeat_interval_ms = cluster_prop_ms(seal_ms, transport.heartbeat_interval_ms);
    }
    Ok(seal_ms)
}

pub(crate) fn seal_drift_adjust_ms(
    current_ms: u64,
    actual_ms: u64,
    expected_ms: u64,
) -> (u64, i64) {
    if current_ms <= 1 || expected_ms == 0 || actual_ms == expected_ms {
        return (current_ms.max(1), 0);
    }
    let cap = current_ms.saturating_mul(SEAL_DRIFT_STEP_PPM) / PPM_DENOM;
    if cap == 0 {
        return (current_ms, 0);
    }
    let next = if actual_ms > expected_ms {
        current_ms.saturating_sub(cap).max(1)
    } else {
        current_ms.saturating_add(cap)
    };
    let delta = next as i128 - current_ms as i128;
    let ppm = delta.saturating_mul(PPM_DENOM as i128) / current_ms as i128;
    (next, ppm as i64)
}

/// True when |actual-expected|/expected < 0.1% (deadband). Skips drift adjust to avoid
/// jitter-driven oscillation; nominal-aligned ticks stay anchored at current cadence.
pub(crate) fn seal_drift_in_deadband(actual_ms: u64, expected_ms: u64) -> bool {
    if expected_ms == 0 {
        return false;
    }
    let diff = actual_ms.max(expected_ms) - actual_ms.min(expected_ms);
    diff.saturating_mul(PPM_DENOM) / expected_ms < SEAL_DRIFT_DEADBAND_PPM
}

/// Clamps `effective_ms` to the owner ±1% envelope around `nominal_ms`. Returns
/// the clamped value plus a flag for the log line. Integer math, saturating.
pub(crate) fn seal_drift_clamp_envelope(nominal_ms: u64, effective_ms: u64) -> (u64, bool) {
    if nominal_ms == 0 {
        return (effective_ms.max(1), false);
    }
    let lo = nominal_ms.saturating_mul(SEAL_ENVELOPE_LO_NUM) / SEAL_ENVELOPE_DENOM;
    let hi = nominal_ms.saturating_mul(SEAL_ENVELOPE_HI_NUM) / SEAL_ENVELOPE_DENOM;
    let lo = lo.max(1);
    if effective_ms < lo {
        (lo, true)
    } else if effective_ms > hi {
        (hi, true)
    } else {
        (effective_ms, false)
    }
}

/// Reports envelope offset as a signed percentage of `nominal_ms` (post-clamp).
/// Used purely for the `envelope_pct` log field; safe for `nominal_ms == 0`.
pub(crate) fn seal_envelope_pct(nominal_ms: u64, effective_ms: u64) -> f64 {
    if nominal_ms == 0 {
        return 0.0;
    }
    let delta = effective_ms as i128 - nominal_ms as i128;
    (delta as f64) * 100.0 / (nominal_ms as f64)
}

/// Grid-aligned next seal deadline: the first multiple of `nominal_ms` strictly
/// greater than `now_ms`. For `bph=3600` (nominal=1000ms) this snaps every seal
/// attempt onto the next wall-clock second, so operator logs across nodes line
/// up on shared second boundaries.
pub(crate) fn align_next_seal_ms(now_ms: u64, nominal_ms: u64) -> u64 {
    let nominal = nominal_ms.max(1);
    let bucket = now_ms / nominal;
    bucket.saturating_add(1).saturating_mul(nominal)
}

/// True when wall-clock has reached the scheduled seal deadline.
pub(crate) fn should_attempt_seal(now_ms: u64, deadline_ms: u64) -> bool {
    now_ms >= deadline_ms
}

/// Proposer ahead-trigger: once per `next_seal_time_ms`, fire when
/// `now_ms >= next_seal - ahead_ms` but before the seal deadline.
pub(crate) fn should_fire_seal_ahead(
    now_ms: u64,
    next_seal_time_ms: u64,
    ahead_ms: u64,
    fired_for_deadline: Option<u64>,
) -> bool {
    if ahead_ms == 0 {
        return false;
    }
    if fired_for_deadline == Some(next_seal_time_ms) {
        return false;
    }
    let trigger_at = next_seal_time_ms.saturating_sub(ahead_ms);
    now_ms >= trigger_at && now_ms < next_seal_time_ms
}

/// Bounded sleep until the next deadline check (variant C poll scheduler).
/// Always returns at least 1 ms; if the deadline has passed but a gate keeps
/// the proposer from sealing, sleep `poll_ms` so retries are paced.
pub(crate) fn poll_sleep_ms(now_ms: u64, deadline_ms: u64, poll_ms: u64) -> u64 {
    let poll = poll_ms.max(1);
    if now_ms >= deadline_ms {
        return poll;
    }
    let remain = deadline_ms - now_ms;
    remain.min(poll).max(1)
}

/// Gate fast-path is proposer-only and only when the first gate probe failed.
pub(crate) fn gate_recheck_needed(is_prop: bool, gate_ok: bool) -> bool {
    is_prop && !gate_ok
}

/// Returns true when a trusted attester peer is fresh enough to count toward quorum.
/// `connected` means the live `PeerRecord` showed `status == Connected`, and
/// `last_seen_ms` is the latest heartbeat / hello / wire activity time.
pub(crate) fn attester_alive(
    connected: bool,
    last_seen_ms: u64,
    now_ms: u64,
    live_window_ms: u64,
) -> bool {
    if !connected {
        return false;
    }
    if last_seen_ms == 0 {
        return false;
    }
    now_ms.saturating_sub(last_seen_ms) <= live_window_ms
}

/// Cluster proposer preflight decision before running record/cluster_gate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SealPreflight {
    /// Quorum is reachable: continue into propose tick + cluster gate.
    Ready,
    /// No live attester peers: skip propose + gate work, slow-sleep, log throttled.
    WaitingAttester,
}

/// Pure preflight rule: Ready only when at least `quorum_k` live attesters
/// (proposer excluded) are connected. Single-node / disabled cluster maps to
/// Ready via `cluster_enabled=false`.
pub(crate) fn cluster_seal_preflight(
    cluster_enabled: bool,
    live_attesters: u8,
    quorum_k: u8,
) -> SealPreflight {
    if !cluster_enabled {
        return SealPreflight::Ready;
    }
    if live_attesters >= quorum_k {
        SealPreflight::Ready
    } else {
        SealPreflight::WaitingAttester
    }
}

#[derive(Clone, Copy, Debug)]
struct AttSyncCtx {
    now_ms: u64,
    local_h: u64,
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct AttSyncRes {
    pub(crate) live_n: u8,
    pub(crate) sync_n: u8,
    pub(crate) local_h: u64,
    pub(crate) peer_tip_max: u64,
}

/// Counts live and sync-ready cluster attesters.
///
/// Sync-ready means the attester is currently live and has no local fork-lock
/// marker for the proposer's next gate height. Proposer preflight intentionally
/// does **not** hard-gate on `sync_live.tip_h` lag because attest path readiness
/// is protocol-level (attester validates parent apply before signing).
pub(crate) async fn count_sync_ready_attesters(app: &App) -> AttSyncRes {
    let now_ms = crate::current_time_ms().unwrap_or(0);
    let local_h = {
        let g = app.inner.read().await;
        g.chain.tip_h()
    };
    let hs = crate::transport::handshake_read_traced(&app, "lifecycle").await;
    count_sync_ready_hs(
        &hs,
        &app.cluster_cfg.members,
        app.node_instance_id.trim(),
        AttSyncCtx { now_ms, local_h },
    )
}

/// Canonical member-id from a trusted peer: instance_id is the cluster member id.
/// This must match RFC16 attest path logic in transport peer-session.
fn trusted_member_id(peer: &crate::transport::TrustedPeer) -> Option<&str> {
    peer.instance_id
        .as_deref()
        .map(str::trim)
        .filter(|id| !id.is_empty())
}

fn member_in_cfg(member_id: &str, members: &[String]) -> bool {
    members.iter().any(|m| m.trim() == member_id)
}

/// Shared pure counting logic for sync-ready attester preflight.
fn count_sync_ready_hs(
    hs: &crate::transport::HandshakeState,
    members: &[String],
    local_member_id: &str,
    ctx: AttSyncCtx,
) -> AttSyncRes {
    let local_member_id = local_member_id.trim();
    let mut out = AttSyncRes {
        local_h: ctx.local_h,
        ..AttSyncRes::default()
    };
    for (node_id, trusted) in hs.trusted_peers.iter() {
        if !trusted.cluster_attest_enabled {
            continue;
        }
        if !matches!(trusted.cluster_role, ClusterRole::Attester) {
            continue;
        }
        let Some(member_id) = trusted_member_id(trusted) else {
            continue;
        };
        if !local_member_id.is_empty() && member_id == local_member_id {
            continue;
        }
        if !member_in_cfg(member_id, members) {
            continue;
        }
        let Some(rec) = hs.peers.get(node_id) else {
            continue;
        };
        let liveish = crate::transport::is_peer_liveish(&rec.status);
        if !attester_alive(
            liveish,
            rec.last_seen_ms,
            ctx.now_ms,
            ATTESTER_LIVE_WINDOW_MS,
        ) {
            continue;
        }
        out.live_n = out.live_n.saturating_add(1);

        let Some(sync) = hs.sync_live.peers.get(node_id) else {
            out.sync_n = out.sync_n.saturating_add(1);
            continue;
        };
        let peer_h = sync.tip_h;
        if peer_h > 0 {
            out.peer_tip_max = out.peer_tip_max.max(peer_h);
        }
        // Keep lag only as observability input; no hard preflight suppression.
        let _lag = ctx.local_h.saturating_sub(peer_h);
        let fork_lock = sync.fork_h == Some(ctx.local_h.saturating_add(1)) && sync.fork_n >= 3;
        if !fork_lock {
            out.sync_n = out.sync_n.saturating_add(1);
        }
    }
    out
}

/// Percent of suppressed iterations within a window (0.0 when no ticks).
pub(crate) fn compute_suppress_pct(ticks: u64, suppressed: u64) -> f64 {
    if ticks == 0 {
        return 0.0;
    }
    (suppressed as f64) * 100.0 / (ticks as f64)
}

/// True when suppression percent crosses the owner alert threshold (>1.0%).
pub(crate) fn is_suppress_alert(pct: f64) -> bool {
    pct > SEAL_SUPPRESS_ALERT_PCT
}

/// Reason last seal slot was suppressed (operator attribution).
/// Note: `WaitingAttester` is not a slot reason — preflight skips opening a slot
/// when no attester is live, so it cannot appear here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SuppressReason {
    LeaseFence,
    ClusterGate,
    SlotSkipped,
}

impl SuppressReason {
    pub(crate) fn as_tag(self) -> &'static str {
        match self {
            SuppressReason::LeaseFence => "lease_fence",
            SuppressReason::ClusterGate => "cluster_gate",
            SuppressReason::SlotSkipped => "slot_skipped",
        }
    }
}

/// Rolling slot-level counters for proposer seal-loop suppression aggregation.
/// Window is 100s of wall clock; counters reset after each emit. One **slot**
/// corresponds to a single grid deadline attempt, not to every poll iteration —
/// many polls during the same waiting slot still count as one attempt. This is
/// the operator-meaningful denominator (~1 slot/s on bph=3600).
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct SealSuppressWindow {
    /// Slots opened in this window (≤ window_sec / nominal_s in steady state).
    pub slots: u64,
    /// Slots that missed seal deadline (one strike max per slot).
    pub slot_supp: u64,
    /// Sealed heights produced in this window (any cause).
    pub sealed_in: u64,
    /// Gate waits inside attest timeout envelope (A path).
    pub wait_att: u64,
    /// Gate hard timeouts (B path).
    pub gate_to: u64,
    /// Active slot deadline_ms; `None` between attempts.
    pub active_deadline_ms: Option<u64>,
    /// Suppression evaluation start for the active slot.
    pub attempt_start_ms: Option<u64>,
    /// True once a suppression strike was recorded for this active slot.
    pub supp_marked: bool,
    /// Last suppression cause inside this window (best-effort attribution).
    pub last_reason: Option<SuppressReason>,
    /// `tip+1` seal target: at most one wait counter per height per window.
    pub wait_marked_h: Option<u64>,
    /// `tip+1` seal target: at most one timeout counter per height per window.
    pub timeout_marked_h: Option<u64>,
    /// `tip+1` seal target: at most one suppression strike per height per window.
    pub strike_marked_h: Option<u64>,
}

/// One-shot operator logs for cluster gate outcomes (per seal target height).
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct ClusterGateDedup {
    missing_round_h: Option<u64>,
    invalid_binding_h: Option<u64>,
    invalid_proposer_h: Option<u64>,
    quorum_wait_h: Option<u64>,
    quorum_timeout_h: Option<u64>,
    waiting_sync_h: Option<u64>,
}

impl ClusterGateDedup {
    fn claim(slot: &mut Option<u64>, h: u64) -> bool {
        if *slot == Some(h) {
            false
        } else {
            *slot = Some(h);
            true
        }
    }

    pub(crate) fn should_log_missing_round(&mut self, h: u64) -> bool {
        Self::claim(&mut self.missing_round_h, h)
    }

    pub(crate) fn should_log_invalid_binding(&mut self, h: u64) -> bool {
        Self::claim(&mut self.invalid_binding_h, h)
    }

    pub(crate) fn should_log_invalid_proposer(&mut self, h: u64) -> bool {
        Self::claim(&mut self.invalid_proposer_h, h)
    }

    pub(crate) fn should_log_quorum_timeout(&mut self, h: u64) -> bool {
        Self::claim(&mut self.quorum_timeout_h, h)
    }

    pub(crate) fn should_log_quorum_wait(&mut self, h: u64) -> bool {
        Self::claim(&mut self.quorum_wait_h, h)
    }

    pub(crate) fn should_log_wait_sync(&mut self, h: u64) -> bool {
        Self::claim(&mut self.waiting_sync_h, h)
    }

    #[cfg(test)]
    pub(crate) fn reset(&mut self) {
        *self = Self::default();
    }
}

impl SealSuppressWindow {
    pub(crate) fn mk_new() -> Self {
        Self::default()
    }

    /// Opens a slot for `deadline_ms` if it differs from the currently active one.
    /// Returns `true` when a new slot was actually counted.
    pub(crate) fn begin_slot(&mut self, deadline_ms: u64, now_ms: u64) -> bool {
        if self.active_deadline_ms == Some(deadline_ms) {
            return false;
        }
        // A new deadline arrived without finishing the prior slot → count it as
        // a skipped slot only if this slot had no prior suppression strike.
        if self.active_deadline_ms.is_some() && !self.supp_marked {
            self.slot_supp = self.slot_supp.saturating_add(1);
            self.last_reason = Some(SuppressReason::SlotSkipped);
        }
        self.active_deadline_ms = Some(deadline_ms);
        self.attempt_start_ms = Some(now_ms);
        self.supp_marked = false;
        self.slots = self.slots.saturating_add(1);
        true
    }

    /// Records one gate-wait observation for `target_h` (`tip+1`), once per height.
    pub(crate) fn note_wait_for_height(&mut self, target_h: u64) -> bool {
        if self.wait_marked_h == Some(target_h) {
            return false;
        }
        self.wait_marked_h = Some(target_h);
        self.wait_att = self.wait_att.saturating_add(1);
        true
    }

    /// Records one gate-timeout observation for `target_h` (`tip+1`), once per height.
    pub(crate) fn note_to_for_height(&mut self, target_h: u64) -> bool {
        if self.timeout_marked_h == Some(target_h) {
            return false;
        }
        self.timeout_marked_h = Some(target_h);
        self.gate_to = self.gate_to.saturating_add(1);
        true
    }

    /// Records one suppression strike for the active slot once nominal window elapsed.
    /// Returns `true` when a strike was recorded on this call.
    #[cfg(test)]
    pub(crate) fn eval_supp(
        &mut self,
        now_ms: u64,
        nominal_ms: u64,
        reason: SuppressReason,
    ) -> bool {
        self.eval_supp_for_height(now_ms, nominal_ms, reason, None)
    }

    /// Like [`Self::eval_supp`] but refuses a second strike for the same `target_h`.
    pub(crate) fn eval_supp_for_height(
        &mut self,
        now_ms: u64,
        nominal_ms: u64,
        reason: SuppressReason,
        target_h: Option<u64>,
    ) -> bool {
        if let Some(h) = target_h {
            if self.strike_marked_h == Some(h) {
                return false;
            }
        }
        if self.active_deadline_ms.is_none() || self.supp_marked {
            return false;
        }
        let nominal_ms = nominal_ms.max(1);
        let Some(start_ms) = self.attempt_start_ms else {
            self.attempt_start_ms = Some(now_ms);
            return false;
        };
        if now_ms.saturating_sub(start_ms) <= nominal_ms {
            return false;
        }
        self.slot_supp = self.slot_supp.saturating_add(1);
        self.last_reason = Some(reason);
        self.supp_marked = true;
        if let Some(h) = target_h {
            self.strike_marked_h = Some(h);
        }
        self.attempt_start_ms = Some(now_ms.saturating_add(SEAL_POLL_INTERVAL_MS));
        true
    }

    /// Closes the active slot as sealed (success), no suppression strike.
    pub(crate) fn close_sealed(&mut self) {
        self.sealed_in = self.sealed_in.saturating_add(1);
        self.active_deadline_ms = None;
        self.attempt_start_ms = None;
        self.supp_marked = false;
    }

    pub(crate) fn reset(&mut self) {
        // Preserve no in-flight slot across emits: a slot that crossed the
        // window boundary still counts in the closing window.
        *self = Self::default();
    }
}

/// Rolling counters for proposer seal-ahead (eager propose) over the same 100s
/// wall window as [`SealSuppressWindow`].
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct SealAheadWindow {
    /// Ahead triggers that passed attester preflight and set `cluster_prop_nudge`.
    pub fired: u64,
    /// Ahead window reached but preflight was `WaitingAttester`.
    pub preflight_skip: u64,
    /// Sum of `next_seal_time_ms - now_ms` at fire time (for avg lead).
    pub lead_ms_sum: u64,
}

impl SealAheadWindow {
    pub(crate) fn mk_new() -> Self {
        Self::default()
    }

    pub(crate) fn note_fired(&mut self, lead_ms: u64) {
        self.fired = self.fired.saturating_add(1);
        self.lead_ms_sum = self.lead_ms_sum.saturating_add(lead_ms);
    }

    pub(crate) fn note_preflight_skip(&mut self) {
        self.preflight_skip = self.preflight_skip.saturating_add(1);
    }

    pub(crate) fn avg_lead_ms(&self) -> u64 {
        if self.fired == 0 {
            0
        } else {
            self.lead_ms_sum / self.fired
        }
    }

    pub(crate) fn reset(&mut self) {
        *self = Self::default();
    }
}

/// One-line ahead analytics for a 100s window (replaces per-slot INFO spam).
fn emit_ahead_summary(ahead_ms: u64, win: &SealAheadWindow) {
    if ahead_ms == 0 {
        return;
    }
    info!(
        "seal_ahead_summary window_sec={} ahead_ms={} fired={} preflight_skip={} avg_lead_ms={}",
        SEAL_SUPPRESS_WINDOW_SEC,
        ahead_ms,
        win.fired,
        win.preflight_skip,
        win.avg_lead_ms(),
    );
}

/// Emits the operator-visible seal-loop suppression summary for one 100s window.
/// Level escalates to ERROR (red via logging.rs color_level_tag) when suppression
/// exceeds 1.0%; otherwise INFO. On ERROR the last suppression reason tag is
/// appended for operator attribution.
fn init_blocked_reason(phase: InitPhase) -> Option<&'static str> {
    match phase {
        InitPhase::LoadingSnapshot => Some("loading_snapshot"),
        InitPhase::Starting => Some("starting"),
        _ => None,
    }
}

fn prep_log_due(last_at: Instant, now: Instant) -> bool {
    now.duration_since(last_at) >= Duration::from_secs(PREP_SUMMARY_IV_SEC)
}

fn emit_suppress_summary(win: &SealSuppressWindow, blocked_reason: Option<&'static str>) {
    let struck = win.slot_supp;
    let pct = compute_suppress_pct(win.slots, struck);
    let reason_tag = win
        .last_reason
        .map(SuppressReason::as_tag)
        .unwrap_or("none");
    let show_blocked = win.sealed_in == 0 && blocked_reason.is_some();
    if is_suppress_alert(pct) {
        if show_blocked {
            error!(
                "seal_suppression_summary window_sec={} slots={} slots_waited_att={} slots_timeout={} slots_struck={} suppression_pct={:.2} sealed_in_window={} last_reason={} blocked_reason={}",
                SEAL_SUPPRESS_WINDOW_SEC,
                win.slots,
                win.wait_att,
                win.gate_to,
                struck,
                pct,
                win.sealed_in,
                reason_tag,
                blocked_reason.unwrap_or("none"),
            );
        } else {
            error!(
                "seal_suppression_summary window_sec={} slots={} slots_waited_att={} slots_timeout={} slots_struck={} suppression_pct={:.2} sealed_in_window={} last_reason={}",
                SEAL_SUPPRESS_WINDOW_SEC,
                win.slots,
                win.wait_att,
                win.gate_to,
                struck,
                pct,
                win.sealed_in,
                reason_tag,
            );
        }
    } else {
        if show_blocked {
            info!(
                "seal_suppression_summary window_sec={} slots={} slots_waited_att={} slots_timeout={} slots_struck={} suppression_pct={:.2} sealed_in_window={} blocked_reason={}",
                SEAL_SUPPRESS_WINDOW_SEC,
                win.slots,
                win.wait_att,
                win.gate_to,
                struck,
                pct,
                win.sealed_in,
                blocked_reason.unwrap_or("none"),
            );
        } else {
            info!(
                "seal_suppression_summary window_sec={} slots={} slots_waited_att={} slots_timeout={} slots_struck={} suppression_pct={:.2} sealed_in_window={}",
                SEAL_SUPPRESS_WINDOW_SEC,
                win.slots,
                win.wait_att,
                win.gate_to,
                struck,
                pct,
                win.sealed_in,
            );
        }
    }
}

fn snap_startup_mode(backend: &SnapshotBackend, stored: BlocksStored) -> &'static str {
    match backend {
        #[cfg(feature = "clickhouse-snapshot")]
        SnapshotBackend::ClickHouse(_) => "clickhouse",
        SnapshotBackend::JsonFile { .. } => match stored {
            BlocksStored::Epochs => "epochs",
            BlocksStored::Inline => "inline",
        },
    }
}

fn tx_kind(tx: &SignedTx) -> &'static str {
    match tx.body {
        TxBody::Init { .. } => "init",
        TxBody::Transfer { .. } => "transfer",
        TxBody::Stake { .. } => "stake",
        TxBody::Unstake { .. } => "unstake",
        TxBody::BurnMark { .. } => "burn_mark",
        TxBody::ClaimIPv4Batch { .. } => "claim_ipv4_batch",
        TxBody::Export { .. } => "export",
        TxBody::Import { .. } => "import",
        TxBody::Policy { .. } => "policy",
    }
}

fn tx_addrs(tx: &SignedTx) -> Vec<[u8; 32]> {
    let signer = tx.computed_account_id();
    match tx.body {
        TxBody::Transfer { to, .. } | TxBody::Import { to, .. } | TxBody::Export { to, .. } => {
            vec![signer, to]
        }
        TxBody::Policy { target_account, .. } => vec![signer, target_account],
        _ => vec![signer],
    }
}

fn tx_bal_diffs(before: &State, after: &State, tx: &SignedTx) -> Vec<([u8; 32], u128, u128)> {
    tx_addrs(tx)
        .into_iter()
        .filter_map(|acc| {
            let prev = before
                .accounts
                .get(&acc)
                .map(|v| v.balance_pwm)
                .unwrap_or(0);
            let next = after.accounts.get(&acc).map(|v| v.balance_pwm).unwrap_or(0);
            (prev != next).then_some((acc, prev, next))
        })
        .collect()
}

fn seal_entry_hash(entry: &SealEntry) -> [u8; 32] {
    match entry {
        SealEntry::Raw(tx) | SealEntry::PreValidated { tx, .. } => tx.tx_hash(),
    }
}

fn dedup_seal_entries(entries: &mut Vec<SealEntry>) {
    let mut seen = HashSet::new();
    entries.retain(|entry| seen.insert(seal_entry_hash(entry)));
}

fn skip_evicted_entries(entries: &mut Vec<SealEntry>, evicted: &HashSet<[u8; 32]>) {
    entries.retain(|entry| !evicted.contains(&seal_entry_hash(entry)));
}

fn first_bad_tx_ctx(
    st: &State,
    txs: &[SignedTx],
    blk_h: u64,
    blk_ts: u64,
    gen_cfg: &pwm_core::GenCfg,
) -> Option<(usize, String)> {
    let mut sim = st.clone();
    for (i, tx) in txs.iter().enumerate() {
        let apply_scope = perfmon::PERF_STATE_APPLY.begin();
        let apply_result = sim.apply_tx_with_ctx(tx, blk_h, blk_ts, gen_cfg);
        apply_scope.end(apply_result.is_ok());
        if let Err(err) = apply_result {
            return Some((i, err.to_string()));
        }
    }
    None
}

fn log_tx_debug(before: &State, after: &State, height: u64, txs: &[SignedTx]) {
    let log = crate::logger();
    for tx in txs {
        let tx_id = hex::encode(tx.tx_hash());
        for (acc, prev, next) in tx_bal_diffs(before, after, tx) {
            log.debug_tx(height, tx_kind(tx), &tx_id, &hex::encode(acc), prev, next);
        }
    }
}

fn log_tx_commit_delta(before: &State, after: &State, height: u64, txs: &[SignedTx]) {
    if !tracing::enabled!(tracing::Level::DEBUG) {
        let _ = height; // height is included by logs elsewhere; keep signature symmetry with debug.
        return;
    }

    for tx in txs {
        let tx_id = hex::encode(tx.tx_hash());
        let sender = tx.computed_account_id();
        let (bal_before, nonce_before) = before
            .get(&sender)
            .map(|a| (a.balance_pwm, a.nonce))
            .unwrap_or((0, 0));
        let (bal_after, nonce_after) = after
            .get(&sender)
            .map(|a| (a.balance_pwm, a.nonce))
            .unwrap_or((0, 0));

        // Keep wording aligned with `/v1/tx`'s export/import commit logs.
        debug!(
            "tx commit delta: kind={} tx_id={} sender={} bal:{}->{} nonce:{}->{}",
            tx_kind(tx),
            tx_id,
            hex::encode(sender),
            bal_before,
            bal_after,
            nonce_before,
            nonce_after
        );
    }
    let _ = height; // height is included by logs elsewhere; keep signature symmetry with debug.
}

fn runtime_mode_summary(mode: &RuntimeIdentityMode) -> String {
    match mode {
        RuntimeIdentityMode::Explicit => "shard_enforced(explicit-domain-config)".to_string(),
        RuntimeIdentityMode::Neutral => "relay_baseline(neutral-default)".to_string(),
    }
}

fn derive_seal_role(config: &PwmdConfig) -> SealRole {
    if let Some(role) = config.seal_role_override {
        return role;
    }
    if config.cluster.enabled && matches!(config.cluster.role, ClusterRole::Attester) {
        return SealRole::Standby;
    }
    if config.debug_disable_seal_loop {
        SealRole::Standby
    } else {
        SealRole::Active
    }
}

async fn req_graceful_stop(app: &App, height: u64, stop_h: u64) {
    let res = crate::api::handlers_shutdown::graceful_shutdown_request(
        app,
        crate::api::handlers_shutdown::ShutdownReason::DebugStop,
    )
    .await;
    if let Err(e) = res {
        error!(
            "debug-stop-height persist failed at height={} stop_h={}: {}",
            height, stop_h, e
        );
    }
    info!(
        "debug-stop-height reached; graceful shutdown triggered at height={} stop_h={}",
        height, stop_h
    );
}

async fn apply_snapshot_init_state(
    app: &App,
    path: Option<std::path::PathBuf>,
    result: Result<(), String>,
    height: u64,
) {
    match result {
        Ok(()) => {
            app.last_snapshot_height.store(height, Ordering::Release);
            let mut st = app.init.write().await;
            *st = InitState::ready(path);
        }
        Err(e) => {
            error!(
                "snapshot save after seal failed path={} height={}: {}",
                path.as_ref()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| "-".into()),
                height,
                e
            );
            let mut st = app.init.write().await;
            *st = InitState::ready_degraded(path, e);
        }
    }
}

pub(crate) fn periodic_snap_save(
    backend: &SnapshotBackend,
    g: &crate::state::Inner,
    height: u64,
    source: &'static str,
    writer: Option<&BlockWriter>,
) -> Option<(Option<std::path::PathBuf>, Result<(), String>)> {
    if !autosnap_hit(height) {
        return None;
    }
    info!(
        "autosnapshot checkpoint hit source={} interval={} height={}",
        source, AUTOSNAPSHOT_BLOCK_INTERVAL, height
    );
    let path = backend.init_state_path();
    let mode = if let Some(writer) = writer {
        if let Err(err) = writer.flush() {
            return Some((path, Err(err)));
        }
        SealPersistMode::PeriodicSummary
    } else {
        SealPersistMode::Periodic
    };
    Some((path, backend.save_seal_persist(g, mode)))
}

fn enqueue_sealed_block(app: &App, block: Arc<Block>) -> Result<(), String> {
    let Some(writer) = app.block_writer.as_ref() else {
        return Ok(());
    };
    if let Err(enqueue_err) = writer.enqueue(Arc::clone(&block)) {
        let backend = app.autosnapshot_backend.as_ref().ok_or_else(|| {
            format!("block enqueue failed without snapshot backend: {enqueue_err}")
        })?;
        backend.recover_append(&block).map_err(|recovery_err| {
            format!(
                "block enqueue failed: {enqueue_err}; synchronous recovery failed: {recovery_err}"
            )
        })?;
    }
    Ok(())
}

pub(crate) async fn periodic_snap_finish(
    app: &App,
    height: u64,
    bak_opt: Option<crate::api::common::CommitBak>,
    save_result: Option<(Option<std::path::PathBuf>, Result<(), String>)>,
) {
    let Some((path, result)) = save_result else {
        return;
    };
    match result {
        Ok(()) => {
            info!(
                "autosnapshot checkpoint summary saved checkpoint_height={}",
                height
            );
            apply_snapshot_init_state(app, path, Ok(()), height).await;
        }
        Err(e) => {
            if let Some(bak) = bak_opt {
                let mut g = app.inner.write().await;
                rollback_commit(&mut g, bak);
                drop(g);
            }
            apply_snapshot_init_state(app, path, Err(e), height).await;
        }
    }
}

fn now_unix_ms() -> u64 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(v) => v.as_millis() as u64,
        Err(_) => 0,
    }
}

async fn maybe_align_mid(app: &App) {
    if !app.debug_align_mid {
        return;
    }
    let wait_ms = mid_wait_ms(now_unix_ms());
    if wait_ms == 0 {
        return;
    }
    tokio::time::sleep(std::time::Duration::from_millis(wait_ms)).await;
}

pub(crate) async fn run_lease_gate(app: &App) -> bool {
    if app.deployment_profile != DeploymentProfile::SingleSealer {
        return true;
    }
    let (tip_h, now_ms) = {
        let g = app.inner.read().await;
        (g.chain.tip_h(), now_unix_ms())
    };
    let mut rt = match app.lease_runtime.lock() {
        Ok(v) => v,
        Err(_) => {
            warn!("seal_suppressed_by_fence reason=lease_runtime_poisoned");
            return false;
        }
    };
    let step = step_lease(
        &app.validator_identity_hash,
        &app.node_instance_id,
        now_ms,
        tip_h,
        app.lease_cfg,
        &mut rt,
        app.lease_backend.as_ref(),
    );
    if let Ok(mut slot) = app.lease_last_err.lock() {
        if rt.last_reason.starts_with("lease_backend_error ") {
            *slot = Some(rt.last_reason.clone());
        } else if step.allow_seal {
            *slot = None;
        }
    }
    if let Some(ev) = step.event {
        app.lease_stats.on_event(ev);
        match ev {
            LeaseEvent::Acquire => {
                info!(
                    "seal_lease_acquired owner={} term={} fence={} expires_at_ms={} tip_h={}",
                    rt.owner_id, rt.term, rt.fence, rt.expires_at_ms, rt.last_tip
                );
            }
            LeaseEvent::Renew => {
                let h = rt.last_tip;
                // Cadence matches `sealed height={}` (first block + every 10) — renewals are per-block noise otherwise.
                if lease_renew_log_hit(&app.lease_renew_log_tip, h) {
                    info!(
                        "seal_lease_renewed owner={} term={} fence={} expires_at_ms={} tip_h={}",
                        rt.owner_id, rt.term, rt.fence, rt.expires_at_ms, h
                    );
                } else if h == 1 || h % 10 == 0 {
                    debug!("seal_lease_renewed_duplicate_suppressed tip_h={}", h);
                }
            }
            LeaseEvent::Takeover => {
                info!(
                    "seal_takeover_committed owner={} term={} fence={} expires_at_ms={} tip_h={}",
                    rt.owner_id, rt.term, rt.fence, rt.expires_at_ms, rt.last_tip
                );
            }
            LeaseEvent::Loss => {
                warn!("seal_lease_lost reason={}", rt.last_reason);
            }
            LeaseEvent::Reject => {
                if rt.last_reason.contains("cas_miss") {
                    if let Some(suppressed) =
                        lease_reject_warn_suppressed(now_ms, rt.last_reason.as_str())
                    {
                        warn!(
                            "seal_lease_cas_failed reason={} suppressed={}",
                            rt.last_reason, suppressed
                        );
                    }
                }
                info!("seal_suppressed_by_fence reason={}", rt.last_reason);
            }
        }
    }
    step.allow_seal
}

pub(crate) async fn run_cluster_gate(
    app: &App,
    mut log_dedup: Option<&mut ClusterGateDedup>,
) -> bool {
    if !app.cluster_cfg.enabled {
        return true;
    }
    let next_h = {
        let g = app.inner.read().await;
        g.chain.tip_h().saturating_add(1)
    };
    let round = 0u32;
    let hs = crate::transport::handshake_read_traced(&app, "lifecycle").await;
    let Some(state) = hs.cluster_attest.rounds.get(&(next_h, round)) else {
        let log_ok = log_dedup
            .as_deref_mut()
            .map(|d| d.should_log_missing_round(next_h))
            .unwrap_or(true);
        if log_ok {
            if next_h <= 2 {
                info!(
                    "seal_suppressed_by_cluster reason=quorum_pending detail=missing_round_state height={} round={} k={} n={} phase=startup",
                    next_h,
                    round,
                    app.cluster_cfg.quorum_k,
                    app.cluster_cfg.quorum_n
                );
            } else {
                warn!(
                    "seal_suppressed_by_cluster reason=quorum_pending detail=missing_round_state height={} round={} k={} n={}",
                    next_h,
                    round,
                    app.cluster_cfg.quorum_k,
                    app.cluster_cfg.quorum_n
                );
            }
        }
        return false;
    };
    let vote_ok = !state.vote_object.trim().is_empty();
    let cand_ok = !state.candidate_hash.trim().is_empty();
    if !vote_ok || !cand_ok {
        let log_ok = log_dedup
            .as_deref_mut()
            .map(|d| d.should_log_invalid_binding(next_h))
            .unwrap_or(true);
        if log_ok {
            warn!(
                "seal_suppressed_by_cluster reason=invalid_proposal detail=binding_incomplete height={} round={} vote_object_present={} candidate_hash_present={}",
                next_h,
                round,
                vote_ok,
                cand_ok
            );
        }
        return false;
    }
    let proposer_ok = state
        .proposer_id
        .as_deref()
        .is_some_and(|id| app.cluster_cfg.members.iter().any(|m| m == id));
    if !proposer_ok {
        let log_ok = log_dedup
            .as_deref_mut()
            .map(|d| d.should_log_invalid_proposer(next_h))
            .unwrap_or(true);
        if log_ok {
            warn!(
                "seal_suppressed_by_cluster reason=invalid_proposal detail=proposer_not_member height={} round={}",
                next_h, round
            );
        }
        return false;
    }
    // RFC16 §7: `k` counts distinct attester ACKs and excludes the proposer.
    let proposer = state.proposer_id.as_deref().unwrap_or_default();
    let ack_n = state
        .attesters
        .keys()
        .filter(|id| {
            let member = id.as_str();
            member != proposer && app.cluster_cfg.members.iter().any(|m| m == member)
        })
        .count() as u8;
    let propose_opened_at_ms = state.propose_opened_at_ms;
    drop(hs);
    if ack_n < app.cluster_cfg.quorum_k {
        let now_ms = crate::current_time_ms().unwrap_or(0);
        if let Some(t0) = propose_opened_at_ms {
            if now_ms.saturating_sub(t0) > app.cluster_cfg.attest_timeout_ms {
                if ack_n == 0 {
                    if let Some(retry_n) = crate::transport::maybe_retry_round(
                        app,
                        next_h,
                        round,
                        CLUSTER_PROP_RETRY_CAP,
                    )
                    .await
                    {
                        app.cluster_prop_nudge
                            .store(true, std::sync::atomic::Ordering::Release);
                        warn!(
                            "cluster_gate_round_reopen height={} round={} reason=got_zero retry={}/{}",
                            next_h,
                            round,
                            retry_n,
                            CLUSTER_PROP_RETRY_CAP
                        );
                        return false;
                    }
                }
                let log_ok = log_dedup
                    .as_deref_mut()
                    .map(|d| d.should_log_quorum_timeout(next_h))
                    .unwrap_or(true);
                if log_ok {
                    warn!(
                        "seal_suppressed_by_cluster reason=quorum_timeout detail=attestations_missing height={} round={} got={} need={} elapsed_ms={} limit_ms={}",
                        next_h,
                        round,
                        ack_n,
                        app.cluster_cfg.quorum_k,
                        now_ms.saturating_sub(t0),
                        app.cluster_cfg.attest_timeout_ms
                    );
                }
                return false;
            }
        }
        let log_ok = log_dedup
            .as_deref_mut()
            .map(|d| d.should_log_quorum_wait(next_h))
            .unwrap_or(true);
        if log_ok {
            debug!(
                "seal_suppressed_by_cluster reason=quorum_pending detail=attestations_missing height={} round={} got={} need={} phase=pre_timeout",
                next_h,
                round,
                ack_n,
                app.cluster_cfg.quorum_k
            );
        }
        return false;
    }
    true
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GateBlock {
    Wait,
    Timeout,
}

pub(crate) async fn run_gate_obs(app: &App) -> Option<GateBlock> {
    if !app.cluster_cfg.enabled || !matches!(app.cluster_cfg.role, ClusterRole::Proposer) {
        return None;
    }
    let next_h = {
        let g = app.inner.read().await;
        g.chain.tip_h().saturating_add(1)
    };
    let round = 0u32;
    let hs = crate::transport::handshake_read_traced(&app, "lifecycle").await;
    let Some(state) = hs.cluster_attest.rounds.get(&(next_h, round)) else {
        return Some(GateBlock::Wait);
    };
    let proposer = state.proposer_id.as_deref().unwrap_or_default();
    let ack_n = state
        .attesters
        .keys()
        .filter(|id| {
            let member = id.as_str();
            member != proposer && app.cluster_cfg.members.iter().any(|m| m == member)
        })
        .count() as u8;
    if ack_n >= app.cluster_cfg.quorum_k {
        return None;
    }
    let now_ms = crate::current_time_ms().unwrap_or(0);
    if let Some(t0) = state.propose_opened_at_ms {
        if now_ms.saturating_sub(t0) > app.cluster_cfg.attest_timeout_ms {
            return Some(GateBlock::Timeout);
        }
    }
    Some(GateBlock::Wait)
}

pub(crate) async fn local_prod_for_h(app: &App, h: u64) -> Result<bool, String> {
    let local_id = app.node_instance_id.trim();
    if local_id.is_empty() {
        return Ok(false);
    }
    let g = app.inner.read().await;
    let mut st = g.chain.st.clone();
    roll_epoch_if_needed(&g.chain.cfg, &mut st, h);
    let prod_idx = pick_prod_idx(h, &st.active_validator_indices)? as usize;
    Ok(app.cluster_cfg.members.get(prod_idx).map(String::as_str) == Some(local_id))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ProdPickFatalDiag {
    tip_h: u64,
    lead_h: u64,
    min_stake: u128,
    max_val_stake: u128,
    val_n: usize,
}

async fn mk_pick_fatal_diag(app: &App, lead_h: u64, err: &str) -> Option<ProdPickFatalDiag> {
    if !err.contains(PROD_PICK_EMPTY_ERR) {
        return None;
    }
    let g = app.inner.read().await;
    if !g.chain.st.active_validator_indices.is_empty() {
        return None;
    }
    let max_val_stake = g
        .chain
        .cfg
        .vals
        .set
        .iter()
        .filter_map(|row| {
            g.chain
                .st
                .accounts
                .get(&row.acct)
                .map(|acc| acc.staked_pwm_raw)
        })
        .max()
        .unwrap_or(0);
    Some(ProdPickFatalDiag {
        tip_h: g.chain.tip_h(),
        lead_h,
        min_stake: g.chain.cfg.min_validator_stake,
        max_val_stake,
        val_n: g.chain.cfg.vals.set.len(),
    })
}

fn exit_fatal_pick(_app: &App, err: &str, diag: ProdPickFatalDiag) -> ! {
    error!(
        "fatal_protocol_blocker role=proposer reason=empty_active_validator_set detail={} tip_h={} lead_h={} min_validator_stake={} max_genesis_validator_stake={} validators={} hint='ensure at least one validator has stake >= min_validator_stake before sealing'",
        err,
        diag.tip_h,
        diag.lead_h,
        diag.min_stake,
        diag.max_val_stake,
        diag.val_n
    );
    error!(
        "pwmd exiting: proposer cannot seal with empty active validator set; requires validator stake/config fix"
    );
    std::process::exit(1);
}

pub(crate) async fn skip_missed_h(app: &App, h: u64) -> bool {
    let mut g = app.inner.write().await;
    if g.chain.tip_h().saturating_add(1) != h {
        return false;
    }
    g.chain.set_canon_h(h);
    true
}

pub(crate) async fn seal_manual_paused(app: &App) -> bool {
    if !app.cluster_cfg.enabled || !matches!(app.cluster_cfg.role, ClusterRole::Proposer) {
        return false;
    }
    app.seal_manual.read().await.mode.is_manual_rpc()
}

pub fn spawn_seal_loop(app: App) {
    tokio::spawn(async move {
        // Best-effort only: Tokio may migrate this task to another worker thread after awaits.
        let _ = thread_priority::set_current_thread_priority(thread_priority::ThreadPriority::Max);
        let bph = {
            let g = app.inner.read().await;
            g.chain.cfg.blocks_per_hour
        };
        let interval_ms = match seal_interval_ms(bph) {
            Ok(ms) => ms,
            Err(err) => {
                error!("seal loop disabled: {}", err);
                return;
            }
        };
        info!(
            "seal_cadence genesis_blocks_per_hour={} seal_interval_ms={}",
            bph, interval_ms
        );
        let nominal_ms = interval_ms;
        // Owner re-anchor at process start: effective always begins at nominal
        // so the ±1% envelope cannot inherit pre-restart windup (RFC: owner invariant).
        // Note: variant C scheduler — `effective_ms` is no longer the cadence driver
        // (no sleep(effective_ms) at loop head). It is still kept as a `seal_cadence_drift`
        // observable so soak operators see realized wall vs expected per 100-block window.
        let mut effective_ms = nominal_ms;
        let mut drift_window_h = {
            let g = app.inner.read().await;
            g.chain.tip_h()
        };
        let mut drift_window_at = Instant::now();
        let mut pending_ticks_since_last_sealed = 0u64;
        // Seal turn watchdog: count deadline-phase loop passes ("microcycles") per turn
        // and raise a one-shot alert when the turn is spinning too long.
        let mut turn_watch_deadline_ms: Option<u64> = None;
        let mut turn_watch_microcycles: u32 = 0;
        let mut turn_watch_alerted = false;
        let mut seal_pt = block_timing::ProfileTime::default();
        let mut seal_turn_start_at: Option<Instant> = None;
        // Owner observability: aggregate suppression % over a 100s wall window.
        // Active only when this node is a cluster proposer; attester/follower paths return early
        // before any of these counters are touched.
        let is_proposer =
            app.cluster_cfg.enabled && matches!(app.cluster_cfg.role, ClusterRole::Proposer);
        let mut supp_win = SealSuppressWindow::mk_new();
        let mut ahead_win = SealAheadWindow::mk_new();
        let mut gate_log_dedup = ClusterGateDedup::default();
        let mut supp_at = Instant::now();
        let mut prep_summary_at = Instant::now() - Duration::from_secs(PREP_SUMMARY_IV_SEC);
        let seal_ahead_ms_cfg = app.cluster_cfg.seal_ahead_ms;
        let mut attester_was_ready = false;
        let mut ahead_fired_for: Option<u64> = None;
        let mut miss_watch_h: Option<u64> = None;
        let mut miss_watch_at_ms = 0u64;
        // Variant C deadline scheduler: poll wall-clock until `next_seal_time_ms`,
        // grid-aligned to multiples of `nominal_ms`. On `bph=3600` this means seals
        // are attempted on second boundaries. Gates re-check every poll without
        // sleeping a full nominal between attempts.
        let mut next_seal_time_ms =
            align_next_seal_ms(crate::current_time_ms().unwrap_or(0), nominal_ms);
        let mut evicted_hashes = HashSet::new();
        let mut evicted_tip = drift_window_h;
        info!(
            "seal_scheduler mode=deadline_poll poll_ms={} nominal_ms={} grid=multiples_of_nominal next_seal_time_ms={}",
            SEAL_POLL_INTERVAL_MS, nominal_ms, next_seal_time_ms
        );
        loop {
            if app
                .shutdown_requested
                .load(std::sync::atomic::Ordering::Acquire)
            {
                info!("seal loop exiting: shutdown requested");
                break;
            }
            let now_ms = crate::current_time_ms().unwrap_or(0);
            if let Some(bt) = app.block_timing.as_ref() {
                let _ = block_timing::try_flush_once(bt);
            }
            if seal_manual_paused(&app).await {
                tokio::time::sleep(Duration::from_millis(SEAL_POLL_INTERVAL_MS)).await;
                continue;
            }
            if is_proposer && seal_ahead_ms_cfg > 0 {
                if should_fire_seal_ahead(
                    now_ms,
                    next_seal_time_ms,
                    seal_ahead_ms_cfg,
                    ahead_fired_for,
                ) {
                    let att = count_sync_ready_attesters(&app).await;
                    let preflight =
                        cluster_seal_preflight(true, att.sync_n, app.cluster_cfg.quorum_k);
                    if matches!(preflight, SealPreflight::Ready) {
                        app.cluster_prop_nudge
                            .store(true, std::sync::atomic::Ordering::Release);
                        record_cluster_prop_tick(&app).await;
                        ahead_fired_for = Some(next_seal_time_ms);
                        let lead_ms = next_seal_time_ms.saturating_sub(now_ms);
                        ahead_win.note_fired(lead_ms);
                    } else {
                        ahead_win.note_preflight_skip();
                    }
                }
            }
            if !should_attempt_seal(now_ms, next_seal_time_ms) {
                turn_watch_deadline_ms = None;
                turn_watch_microcycles = 0;
                turn_watch_alerted = false;
                let sleep_ms = poll_sleep_ms(now_ms, next_seal_time_ms, SEAL_POLL_INTERVAL_MS);
                if is_proposer {
                    tokio::select! {
                        _ = app.seal_wake.notified() => {},
                        _ = tokio::time::sleep(Duration::from_millis(sleep_ms)) => {},
                    }
                } else {
                    tokio::time::sleep(Duration::from_millis(sleep_ms)).await;
                }
                continue;
            }
            if turn_watch_deadline_ms != Some(next_seal_time_ms) {
                turn_watch_deadline_ms = Some(next_seal_time_ms);
                turn_watch_microcycles = 0;
                turn_watch_alerted = false;
            }
            turn_watch_microcycles = turn_watch_microcycles.saturating_add(1);
            if is_proposer && !turn_watch_alerted && turn_watch_microcycles > 20 {
                warn!(
                    "seal_turn_watchdog stage=attempt_phase microcycles={} deadline_ms={} now_ms={} pending_ticks_since_last_sealed={}",
                    turn_watch_microcycles,
                    next_seal_time_ms,
                    now_ms,
                    pending_ticks_since_last_sealed
                );
                turn_watch_alerted = true;
            }
            if is_proposer && supp_at.elapsed() >= Duration::from_secs(SEAL_SUPPRESS_WINDOW_SEC) {
                let init_phase = app.init.read().await.phase;
                let blocked_reason = init_blocked_reason(init_phase);
                emit_suppress_summary(&supp_win, blocked_reason);
                supp_win.reset();
                emit_ahead_summary(seal_ahead_ms_cfg, &ahead_win);
                ahead_win.reset();
                supp_at = Instant::now();
            }
            // Deadline reached: every suppress-continue below pauses one poll tick before
            // re-checking the gate so the loop does not spin on a stale deadline.
            let poll_pause = Duration::from_millis(SEAL_POLL_INTERVAL_MS);
            let init_state = app.init.read().await.clone();
            if !init_state.allows_chain_progress() {
                if is_proposer {
                    let waiting_since = app.cluster_prep_wait_ms.load(Ordering::Acquire);
                    let waiting_since = if waiting_since == 0 {
                        app.cluster_prep_wait_ms.store(now_ms, Ordering::Release);
                        now_ms
                    } else {
                        waiting_since
                    };
                    let loading_sec = now_ms.saturating_sub(waiting_since) / 1000;
                    if prep_log_due(prep_summary_at, Instant::now()) {
                        let local_tip = {
                            let g = app.inner.read().await;
                            g.chain.tip_h()
                        };
                        if let Some(blocked_reason) = init_blocked_reason(init_state.phase) {
                            info!(
                                "cluster_prep_summary phase={} local_tip={} loading_sec={} ready_for_seal=false blocked_reason={} snapshot_file={} snapshot_diag={}",
                                init_state.phase.as_str(),
                                local_tip,
                                loading_sec,
                                blocked_reason,
                                init_state
                                    .snapshot_file
                                    .as_ref()
                                    .map(|v| v.display().to_string())
                                    .unwrap_or_else(|| "none".to_string()),
                                init_state
                                    .snapshot_error
                                    .clone()
                                    .unwrap_or_else(|| "none".to_string()),
                            );
                        }
                        prep_summary_at = Instant::now();
                    }
                }
                tokio::time::sleep(poll_pause).await;
                continue;
            }
            if is_proposer {
                app.cluster_prep_wait_ms.store(0, Ordering::Release);
            }
            if app.debug_disable_seal_loop {
                // Follower / replay-only mode: chain height may advance via sync apply, not local seal.
                // Still honor debug-stop-height so harnesses (e.g. Wave A) can shut down cleanly.
                let h = {
                    let g = app.inner.read().await;
                    g.chain.tip_h()
                };
                if let Some(stop_h) = app.debug_stop_height {
                    if h >= stop_h {
                        req_graceful_stop(&app, h, stop_h).await;
                    }
                }
                tokio::time::sleep(poll_pause).await;
                continue;
            }
            if app.cluster_cfg.enabled && matches!(app.cluster_cfg.role, ClusterRole::Attester) {
                // RFC16: attester is non-committer; no local seal/cluster gate polling.
                let h = {
                    let g = app.inner.read().await;
                    g.chain.tip_h()
                };
                if let Some(stop_h) = app.debug_stop_height {
                    if h >= stop_h {
                        req_graceful_stop(&app, h, stop_h).await;
                    }
                }
                tokio::time::sleep(poll_pause).await;
                continue;
            }
            if is_proposer {
                // Preflight: do not propose / poll the cluster gate while no attester peer
                // is live. This kills the startup wall of `missing_round_state` warnings
                // and prevents the suppression denominator from inflating before quorum
                // is even reachable.
                let att = count_sync_ready_attesters(&app).await;
                let preflight = cluster_seal_preflight(true, att.sync_n, app.cluster_cfg.quorum_k);
                if matches!(preflight, SealPreflight::WaitingAttester) {
                    let gate_h = att.local_h.saturating_add(1).max(1);
                    let waiting_since = app.cluster_prep_wait_ms.load(Ordering::Acquire);
                    let waiting_since = if waiting_since == 0 {
                        app.cluster_prep_wait_ms.store(now_ms, Ordering::Release);
                        now_ms
                    } else {
                        waiting_since
                    };
                    let waiting_sec = now_ms.saturating_sub(waiting_since) / 1000;
                    let should_warn = gate_log_dedup.should_log_wait_sync(gate_h);
                    if should_warn {
                        warn!(
                            "cluster_attest_waiting_sync height={} live_synced_attesters={} live_connected_attesters={} quorum_k={} cluster_n={} max_tip_lag={} proposer_tip={} attester_tip_max={} waiting_sec={}",
                            gate_h,
                            att.sync_n,
                            att.live_n,
                            app.cluster_cfg.quorum_k,
                            app.cluster_cfg.quorum_n,
                            app.cluster_cfg.att_max_tip_lag,
                            att.local_h,
                            att.peer_tip_max,
                            waiting_sec
                        );
                    }
                    if prep_log_due(prep_summary_at, Instant::now()) {
                        info!(
                            "cluster_prep_summary phase=waiting_attester live_synced_attesters={} live_connected={} proposer_tip={} attester_tip_max={} max_tip_lag={} waiting_sec={} ready_for_seal=false blocked_reason=waiting_attester_quorum",
                            att.sync_n,
                            att.live_n,
                            att.local_h,
                            att.peer_tip_max,
                            app.cluster_cfg.att_max_tip_lag,
                            waiting_sec
                        );
                        prep_summary_at = Instant::now();
                    }
                    attester_was_ready = false;
                    tokio::time::sleep(Duration::from_millis(SEAL_WAIT_PEER_MS)).await;
                    continue;
                }
                app.cluster_prep_wait_ms.store(0, Ordering::Release);
                if !attester_was_ready {
                    info!(
                        "cluster_attest_ready live_synced_attesters={} live_connected_attesters={} quorum_k={} max_tip_lag={}",
                        att.sync_n,
                        att.live_n,
                        app.cluster_cfg.quorum_k,
                        app.cluster_cfg.att_max_tip_lag
                    );
                    attester_was_ready = true;
                }
                let lead_h = {
                    let g = app.inner.read().await;
                    g.chain.tip_h().saturating_add(1)
                };
                match local_prod_for_h(&app, lead_h).await {
                    Ok(true) => {
                        miss_watch_h = None;
                    }
                    Ok(false) => {
                        if miss_watch_h != Some(lead_h) {
                            miss_watch_h = Some(lead_h);
                            miss_watch_at_ms = now_ms;
                            info!(
                                "cluster_primary_wait height={} local_proposer={} window_ms={}",
                                lead_h, app.node_instance_id, nominal_ms
                            );
                        }
                        if now_ms.saturating_sub(miss_watch_at_ms) >= nominal_ms {
                            if skip_missed_h(&app, lead_h).await {
                                warn!(
                                    "cluster_primary_miss height={} local_proposer={} action=skip_to_failover",
                                    lead_h,
                                    app.node_instance_id
                                );
                                miss_watch_h = None;
                                next_seal_time_ms = align_next_seal_ms(now_ms, nominal_ms);
                            }
                        }
                        tokio::time::sleep(poll_pause).await;
                        continue;
                    }
                    Err(err) => {
                        if let Some(diag) = mk_pick_fatal_diag(&app, lead_h, &err).await {
                            exit_fatal_pick(&app, &err, diag);
                        }
                        if miss_watch_h != Some(lead_h) {
                            miss_watch_h = Some(lead_h);
                            miss_watch_at_ms = now_ms;
                            warn!(
                                "cluster_primary_wait height={} local_proposer={} reason=proposer_pick_failed detail={}",
                                lead_h,
                                app.node_instance_id,
                                err
                            );
                        }
                        tokio::time::sleep(poll_pause).await;
                        continue;
                    }
                }
                // Open / re-confirm the current grid slot exactly once per deadline.
                let slot_new = supp_win.begin_slot(next_seal_time_ms, now_ms);
                if slot_new {
                    seal_pt.start(Some(now_ms));
                    seal_turn_start_at = Some(Instant::now());
                    seal_pt.checkpoint_at("slot_open", now_ms);
                    if let Some(bt) = app.block_timing.as_ref() {
                        let g = app.inner.read().await;
                        let h = g.chain.tip_h().saturating_add(1);
                        drop(g);
                        block_timing::note_t0(
                            bt,
                            block_timing::T0Ctx {
                                h,
                                r: 0,
                                t_ms: block_timing::now_ms_f64(),
                                grid_ms: next_seal_time_ms,
                                nom_ms: nominal_ms,
                            },
                        );
                    }
                    record_cluster_prop_tick(&app).await;
                }
            }
            let gate_target_h = {
                let g = app.inner.read().await;
                g.chain.tip_h().saturating_add(1)
            };
            let lease_gate_start_at = Instant::now();
            seal_pt.checkpoint("lease_gate_begin");
            if !run_lease_gate(&app).await {
                seal_pt.checkpoint("lease_gate_blocked");
                if is_proposer {
                    supp_win.eval_supp_for_height(
                        now_ms,
                        nominal_ms,
                        SuppressReason::LeaseFence,
                        Some(gate_target_h),
                    );
                }
                let h = {
                    let g = app.inner.read().await;
                    g.chain.tip_h()
                };
                if let Some(stop_h) = app.debug_stop_height {
                    if h >= stop_h {
                        req_graceful_stop(&app, h, stop_h).await;
                    }
                }
                tokio::time::sleep(poll_pause).await;
                continue;
            }
            seal_pt.checkpoint("lease_gate_ok");
            // Gate fast-path: after deadline, if quorum flips to ready during this
            // same poll tick, re-check once and seal immediately (no extra poll sleep).
            // Invariant preserved: this branch runs only after should_attempt_seal().
            let cluster_gate_start_at = Instant::now();
            seal_pt.checkpoint("cluster_gate_begin");
            let mut gate_ok = run_cluster_gate(&app, Some(&mut gate_log_dedup)).await;
            let gate_recheck_used = gate_recheck_needed(is_proposer, gate_ok);
            if gate_recheck_used {
                seal_pt.checkpoint("cluster_gate_recheck_begin");
                gate_ok = run_cluster_gate(&app, Some(&mut gate_log_dedup)).await;
                seal_pt.checkpoint("cluster_gate_recheck_done");
            }
            if !gate_ok {
                seal_pt.checkpoint("cluster_gate_blocked");
                pending_ticks_since_last_sealed = pending_ticks_since_last_sealed.saturating_add(1);
                if is_proposer {
                    match run_gate_obs(&app).await {
                        Some(GateBlock::Wait) => {
                            supp_win.note_wait_for_height(gate_target_h);
                        }
                        Some(GateBlock::Timeout) => {
                            supp_win.note_to_for_height(gate_target_h);
                        }
                        None => {}
                    }
                    supp_win.eval_supp_for_height(
                        now_ms,
                        nominal_ms,
                        SuppressReason::ClusterGate,
                        Some(gate_target_h),
                    );
                }
                let h = {
                    let g = app.inner.read().await;
                    g.chain.tip_h()
                };
                if let Some(stop_h) = app.debug_stop_height {
                    if h >= stop_h {
                        req_graceful_stop(&app, h, stop_h).await;
                    }
                }
                tokio::time::sleep(poll_pause).await;
                continue;
            }
            seal_pt.checkpoint("cluster_gate_ok");
            let cluster_gate_us = cluster_gate_start_at.elapsed().as_micros();
            let lease_gate_us = lease_gate_start_at.elapsed().as_micros();
            if let Some(bt) = app.block_timing.as_ref() {
                block_timing::note_gate_ok(
                    bt,
                    block_timing::SendCtx {
                        h: gate_target_h,
                        r: 0,
                        t_ms: block_timing::now_ms_f64(),
                    },
                );
            }
            // Variant C: capture the next deadline *before* attempting to seal so a post-seal
            // operational delay does not stack into the next interval. Only commit
            // `next_seal_time_ms = scheduled_next` on `Ok(seal)`; retry paths move the
            // deadline by one poll interval to avoid a lock-bound microcycle.
            let scheduled_next =
                align_next_seal_ms(crate::current_time_ms().unwrap_or(0), nominal_ms);
            maybe_align_mid(&app).await;
            seal_pt.checkpoint("before_write_lock");
            let mut g = app.inner.write().await;
            seal_pt.checkpoint("after_write_lock");
            let now_h = g.chain.tip_h();
            if now_h != evicted_tip {
                evicted_hashes.clear();
                evicted_tip = now_h;
            }
            let expired = g.roaming_pool.expire_by_height(now_h);
            if expired > 0 {
                info!("expired roaming intents count={} height={}", expired, now_h);
            }
            let pool_scope = perfmon::PERF_POOL_DRAIN.begin();
            if let Ok(mut rx) = app.tx_ingress.receiver.try_lock() {
                while let Ok(tx) = rx.try_recv() {
                    let _ = g.pool.push(tx);
                }
            }
            let block_cap = 64usize;
            let mut entries = Vec::with_capacity(block_cap);
            if let Ok(mut rx) = app._validated_rx.try_lock() {
                let _span = debug_span!("seal.drain_validated").entered();
                while entries.len() < block_cap {
                    match rx.try_recv() {
                        Ok(validated) => {
                            app.pipeline_metrics.inc_dequeued();
                            if validated.validated_at_height != now_h {
                                app.pipeline_metrics.inc_stale_validated();
                            }
                            entries.push(SealEntry::PreValidated {
                                tx: validated.tx,
                                at_height: validated.validated_at_height,
                            });
                        }
                        Err(_) => break,
                    }
                }
            }
            let remaining = block_cap.saturating_sub(entries.len());
            entries.extend(g.pool.take(remaining).into_iter().map(SealEntry::Raw));
            skip_evicted_entries(&mut entries, &evicted_hashes);
            dedup_seal_entries(&mut entries);
            pool_scope.end(true);
            let st_before = tracing::enabled!(tracing::Level::DEBUG).then(|| g.chain.st.clone());
            let persist_back = app.autosnapshot_backend.is_some() && autosnap_hit(now_h + 1);
            let bak_opt = persist_back.then(|| take_bak(&g));
            seal_pt.checkpoint("before_chain_seal");
            let seal_scope = perfmon::PERF_CHAIN_SEAL.begin();
            let seal_result = g.chain.seal_entries(entries);
            seal_scope.end(seal_result.is_ok());
            match seal_result {
                Ok(()) => {
                    let seal_done_ms = crate::current_time_ms().unwrap_or(now_ms);
                    seal_pt.checkpoint_at("after_chain_seal", seal_done_ms);
                    let sealed_state = Arc::new(g.chain.st.clone());
                    app.state_snapshot.store(sealed_state);
                    app.hot_index.refresh(&g.chain.st);
                    app.pipeline_metrics.finish_block();
                    next_seal_time_ms = scheduled_next;
                    turn_watch_deadline_ms = None;
                    turn_watch_microcycles = 0;
                    turn_watch_alerted = false;
                    let h = g.chain.tip_h();
                    evicted_hashes.clear();
                    evicted_tip = h;
                    app.worker_tip_height
                        .store(h, std::sync::atomic::Ordering::Relaxed);
                    let stop_h = app.debug_stop_height;
                    if is_proposer {
                        const GATE_LOG_THRESHOLD_US: u128 = 1_000;
                        if cluster_gate_us > GATE_LOG_THRESHOLD_US
                            || lease_gate_us > GATE_LOG_THRESHOLD_US
                        {
                            info!(
                                "seal_gate_profile h={} cluster_gate_us={} lease_gate_us={} cluster_gate_ms={:.1} recheck={}",
                                h, cluster_gate_us, lease_gate_us,
                                cluster_gate_us as f64 / 1000.0,
                                gate_recheck_used
                            );
                        } else {
                            debug!(
                                "seal_gate_profile h={} cluster_gate_us={} lease_gate_us={} cluster_gate_ms={:.1} recheck={}",
                                h, cluster_gate_us, lease_gate_us,
                                cluster_gate_us as f64 / 1000.0,
                                gate_recheck_used
                            );
                        }
                        supp_win.close_sealed();
                    }
                    if h == 1 || h % 10 == 0 {
                        info!("sealed height={}", h);
                    }
                    if cluster_pending_summary_hit(h) {
                        info!(
                            "cluster_gate_pending_summary pending_ticks_since_last_sealed={} sealed_h={}",
                            pending_ticks_since_last_sealed,
                            h
                        );
                        pending_ticks_since_last_sealed = 0;
                    }
                    let sealed_block = g.chain.blocks.back().cloned().map(Arc::new);
                    if let Some(blk) = sealed_block.as_deref() {
                        let txs = blk.txs.clone();
                        counters::inc_sealed_by(u64::try_from(txs.len()).unwrap_or(u64::MAX));
                        if let Some(st_before) = st_before.as_ref() {
                            log_tx_debug(st_before, &g.chain.st, h, &txs);
                            log_tx_commit_delta(st_before, &g.chain.st, h, &txs);
                        }
                        for tx in &txs {
                            g.record_cross_shard_tx(tx, h);
                            let _ = app.tx_events.send(TxEvent::Sealed {
                                txid: tx.tx_hash(),
                                block_height: h,
                            });
                        }
                        if !txs.is_empty() {
                            if let Some(bt) = app.block_timing.as_ref() {
                                let wall_total_ms = seal_turn_start_at
                                    .as_ref()
                                    .map(|started_at| {
                                        (started_at.elapsed().as_secs_f64() * 100.0).round() / 100.0
                                    })
                                    .unwrap_or(0.0);
                                block_timing::note_seal(
                                    bt,
                                    block_timing::SealCtx {
                                        h,
                                        r: 0,
                                        seal_ms: block_timing::now_ms_f64(),
                                        wall_total_ms,
                                        pending_ticks: pending_ticks_since_last_sealed,
                                        gate_recheck: gate_recheck_used,
                                        autosnap: persist_back,
                                        supp_strike: supp_win.supp_marked,
                                        attest_to: supp_win.timeout_marked_h == Some(h),
                                        nom_ms: nominal_ms,
                                        grid_ms: next_seal_time_ms,
                                        profile_json: seal_pt
                                            .json_stats_with_precision("{\"scope\":\"seal\"}", 2),
                                    },
                                );
                            }
                        }
                    }
                    if h > 0 && h % SUMMARY_BLOCK_INTERVAL == 0 {
                        info!("{}", summary_log_line(&g.cross_shard.summary()));
                    }
                    drop(g);
                    let append_result = sealed_block
                        .map(|block| enqueue_sealed_block(&app, block))
                        .unwrap_or(Ok(()));
                    let save_result = match append_result {
                        Ok(()) => {
                            if let Some(backend) = app.autosnapshot_backend.as_ref() {
                                let g = app.inner.read().await;
                                periodic_snap_save(
                                    backend,
                                    &g,
                                    h,
                                    "seal",
                                    app.block_writer.as_ref(),
                                )
                            } else {
                                None
                            }
                        }
                        Err(err) => app
                            .autosnapshot_backend
                            .as_ref()
                            .map(|backend| (backend.init_state_path(), Err(err))),
                    };
                    periodic_snap_finish(&app, h, bak_opt, save_result).await;
                    let drift_blocks = h.saturating_sub(drift_window_h);
                    if drift_blocks >= SEAL_DRIFT_WINDOW_BLOCKS {
                        let actual_ms = drift_window_at.elapsed().as_millis() as u64;
                        let expected_ms = drift_blocks.saturating_mul(nominal_ms);
                        // Deadband: skip adjust when drift is below 0.1% to avoid jitter oscillation.
                        // adjust_pct in the log is the per-step adjust fraction (ppm/10_000),
                        // not the envelope offset; envelope_pct is reported separately post-clamp.
                        let (adjusted_ms, adjust_ppm) =
                            if seal_drift_in_deadband(actual_ms, expected_ms) {
                                (effective_ms, 0)
                            } else {
                                seal_drift_adjust_ms(effective_ms, actual_ms, expected_ms)
                            };
                        let (clamped_ms, clamp_applied) =
                            seal_drift_clamp_envelope(nominal_ms, adjusted_ms);
                        let envelope_pct = seal_envelope_pct(nominal_ms, clamped_ms);
                        info!(
                            "seal_cadence_drift blocks={} nominal_ms={} effective_ms={} actual_ms={} expected_ms={} adjust_pct={:.4} envelope_pct={:.4} clamp_applied={}",
                            drift_blocks,
                            nominal_ms,
                            clamped_ms,
                            actual_ms,
                            expected_ms,
                            adjust_ppm as f64 / 10_000.0,
                            envelope_pct,
                            clamp_applied
                        );
                        effective_ms = clamped_ms;
                        drift_window_h = h;
                        drift_window_at = Instant::now();
                    }
                    if let Some(stop_h) = stop_h {
                        if h >= stop_h {
                            req_graceful_stop(&app, h, stop_h).await;
                        }
                    }
                }
                Err((e, txs)) => {
                    let replay = if e.starts_with("tx: ") {
                        let (blk_h, blk_ts) = match g.chain.next_apply_ctx() {
                            Ok(ctx) => ctx,
                            Err(err) => {
                                warn!("seal skip: failed to resolve apply ctx: {}", err);
                                g.pool.prepend_block(txs);
                                next_seal_time_ms = crate::current_time_ms()
                                    .unwrap_or(now_ms)
                                    .saturating_add(SEAL_POLL_INTERVAL_MS);
                                continue;
                            }
                        };
                        let drop_at =
                            first_bad_tx_ctx(&g.chain.st, &txs, blk_h, blk_ts, &g.chain.cfg);
                        if let Some((i, err)) = drop_at {
                            let bad_sender_id = txs[i].computed_account_id();
                            let on_chain_nonce = g
                                .chain
                                .st
                                .get(&bad_sender_id)
                                .map(|account| account.nonce)
                                .unwrap_or(0);
                            evicted_hashes.insert(txs[i].tx_hash());
                            let mut stale_hashes = HashSet::new();
                            for tx in txs[i + 1..].iter() {
                                let sender_id = tx.computed_account_id();
                                if sender_id == bad_sender_id && tx.nonce <= on_chain_nonce {
                                    let tx_hash = tx.tx_hash();
                                    stale_hashes.insert(tx_hash);
                                    evicted_hashes.insert(tx_hash);
                                }
                            }
                            counters::inc_rejected_by(
                                1 + u64::try_from(stale_hashes.len()).unwrap_or(u64::MAX),
                            );
                            if !stale_hashes.is_empty() {
                                warn!(
                                    "seal skip: evicting {} stale same-sender txs sender={}",
                                    stale_hashes.len(),
                                    hex::encode(bad_sender_id)
                                );
                            }
                            warn!(
                                "seal skip: evicting unapplicable tx at index {} ({}), requeueing {} others",
                                i,
                                err,
                                txs.len().saturating_sub(1 + stale_hashes.len())
                            );
                            let mut kept = Vec::with_capacity(txs.len().saturating_sub(1));
                            kept.extend(txs[..i].iter().cloned());
                            kept.extend(
                                txs[i + 1..]
                                    .iter()
                                    .filter(|tx| !stale_hashes.contains(&tx.tx_hash()))
                                    .cloned(),
                            );
                            kept
                        } else {
                            warn!(
                                "seal skip: {} (could not locate failing tx; requeue full batch)",
                                e
                            );
                            // TODO: These are requeued, not permanently dropped; track as rejected for backpressure visibility.
                            counters::inc_rejected_by(u64::try_from(txs.len()).unwrap_or(u64::MAX));
                            txs
                        }
                    } else {
                        warn!("seal skip: {}", e);
                        // TODO: These are requeued, not permanently dropped; track as rejected for backpressure visibility.
                        counters::inc_rejected_by(u64::try_from(txs.len()).unwrap_or(u64::MAX));
                        txs
                    };
                    g.pool.prepend_block(replay);
                    next_seal_time_ms = crate::current_time_ms()
                        .unwrap_or(now_ms)
                        .saturating_add(SEAL_POLL_INTERVAL_MS);
                }
            }
        }
    });
}

fn spawn_shutdown_signal_task(app: App) {
    tokio::spawn(async move {
        #[cfg(unix)]
        {
            use tokio::signal::unix::{signal, SignalKind};

            let mut sigterm = match signal(SignalKind::terminate()) {
                Ok(sig) => sig,
                Err(err) => {
                    error!("shutdown signal setup failed: {}", err);
                    return;
                }
            };
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {
                    let _ = crate::api::handlers_shutdown::graceful_shutdown_request(
                        &app,
                        crate::api::handlers_shutdown::ShutdownReason::Signal("SIGINT"),
                    ).await;
                }
                _ = sigterm.recv() => {
                    let _ = crate::api::handlers_shutdown::graceful_shutdown_request(
                        &app,
                        crate::api::handlers_shutdown::ShutdownReason::Signal("SIGTERM"),
                    ).await;
                }
            }
            return;
        }

        #[cfg(not(unix))]
        {
            if tokio::signal::ctrl_c().await.is_ok() {
                let _ = crate::api::handlers_shutdown::graceful_shutdown_request(
                    &app,
                    crate::api::handlers_shutdown::ShutdownReason::Signal("SIGINT"),
                )
                .await;
            }
        }
    });
}

fn shutdown_if_fatal_snapshot(app: &App, stage: &'static str) {
    if !app.exit_on_fatal_snapshot {
        return;
    }
    error!(
        target: SNAP_STARTUP_TARGET,
        stage,
        "pwmd exiting after unrecoverable snapshot error (PWM_KEEP_ALIVE_ON_SNAPSHOT_ERROR=1 or --keep-alive-on-snapshot-error keeps degraded HTTP)"
    );
    std::process::exit(1);
}

pub(crate) fn spawn_snapshot_loader(app: App) {
    tokio::spawn(async move {
        let Some(backend) = app.autosnapshot_backend.clone() else {
            let mut st = app.init.write().await;
            *st = InitState::ready(app.data_file.clone());
            info!("autosnapshot persistence disabled; using genesis-only chain state");
            crate::logger().info("pwmd startup phase: ready (snapshot persistence disabled)");
            return;
        };
        let loader_start = Instant::now();
        let diag = backend.diag_label();
        let snap_path = backend.init_state_path();
        {
            let mut st = app.init.write().await;
            *st = InitState::loading(snap_path.clone());
        }
        info!("snapshot loading started: {}", diag);
        crate::logger().info(&format!("pwmd startup phase: loading_snapshot ({})", diag));
        let cfg = {
            let g = app.inner.read().await;
            g.chain.cfg.clone()
        };
        let anchor_sk = {
            let g = app.inner.read().await;
            g.chain.val_sks.first().cloned()
        };
        let opts = SnapshotLoadOpts {
            verify_chain: app.snapshot_verify_chain,
            anchor_sk,
            anchor_idx: 0,
        };
        match backend.load(&cfg, opts) {
            Ok((Some(snap), io_ph)) => {
                let stored = snap.blocks_stored;
                let mode_str = snap_startup_mode(&backend, stored);
                let snap_chk_h = snap.checkpoint_height;
                let t_ir = Instant::now();
                let (blocks, state, roaming_pool, cross_shard) = match snap.into_runtime() {
                    Ok(v) => v,
                    Err(e) => {
                        let g0_digest = hex::encode(digest(&cfg.state0()));
                        let elapsed_ms = loader_start.elapsed().as_millis() as u64;
                        warn!(
                            genesis_state0_digest = %g0_digest,
                            err = %e,
                            "snapshot into_runtime failed; compare genesis params and pwmd build with snapshot writer"
                        );
                        error!(
                            target: SNAP_STARTUP_TARGET,
                            stage = "into_runtime",
                            elapsed_ms,
                            err = %e,
                            path = %diag,
                            mode = mode_str,
                            "snapshot startup degraded"
                        );
                        let mut st = app.init.write().await;
                        *st = InitState::ready_degraded(snap_path.clone(), e.clone());
                        crate::logger().error(&format!(
                            "pwmd startup phase: ready_degraded (snapshot error: {e})"
                        ));
                        shutdown_if_fatal_snapshot(&app, "into_runtime");
                        return;
                    }
                };
                let into_runtime_ms = t_ir.elapsed().as_millis() as u64;
                let tip_h = blocks.last().map(|b| b.hdr.height).unwrap_or(0);
                let reg_n = state.exported_registry.len();
                let imp_n = state.imported_set.len();
                let t_abs = Instant::now();
                let mut g = app.inner.write().await;
                g.chain.blocks = absorb_blocks_tail(blocks);
                g.chain.st = state;
                g.chain.sync_canon_h();
                g.roaming_pool = roaming_pool;
                g.cross_shard = cross_shard;
                drop(g);
                let absorb_tail_ms = t_abs.elapsed().as_millis() as u64;
                let mut st = app.init.write().await;
                *st = InitState::ready(snap_path.clone());
                app.last_snapshot_height.store(tip_h, Ordering::Release);
                align_summary_post_verify(&app, &backend, &io_ph, tip_h).await;
                let total_ms = loader_start.elapsed().as_millis() as u64;
                let (summary_read_ms, epochs_ms, validate_ms, ch_http_ms, ch_parse_ms, ch_branch) =
                    match &io_ph {
                        SnapIoTiming::Json(j) => (
                            j.summary_read_ms,
                            j.epochs_ms,
                            j.validate_ms,
                            0u64,
                            0u64,
                            "",
                        ),
                        #[cfg(feature = "clickhouse-snapshot")]
                        SnapIoTiming::Ch(c) => (0u64, 0u64, 0u64, c.http_ms, c.parse_ms, c.branch),
                    };
                info!(
                    target: SNAP_STARTUP_TARGET,
                    path = %diag,
                    mode = mode_str,
                    tip_h,
                    canonical_h = snap_chk_h,
                    total_ms,
                    summary_read_ms,
                    epochs_ms,
                    validate_ms,
                    into_runtime_ms,
                    absorb_tail_ms,
                    ch_http_ms,
                    ch_parse_ms,
                    ch_branch,
                    "snapshot startup load ok"
                );
                info!("snapshot loaded from {}", diag);
                let g0_digest = hex::encode(digest(&cfg.state0()));
                info!(
                    tip_h = tip_h,
                    bridge_exported_registry = reg_n,
                    bridge_imported_set = imp_n,
                    genesis_state0_digest = %g0_digest,
                    "snapshot load: cross-shard bridge counters after apply"
                );
                if reg_n > imp_n {
                    warn!(
                        tip_h = tip_h,
                        pending_registered_minus_imported = reg_n.saturating_sub(imp_n),
                        "snapshot load: exported_registry exceeds imported_set; pending imports may need target relay (see handoff_register and v1_tx import logs)"
                    );
                }
                crate::logger().info("pwmd startup phase: ready (snapshot loaded)");
            }
            Ok((None, _io_ph)) => {
                let mut st = app.init.write().await;
                *st = InitState::ready(snap_path.clone());
                app.last_snapshot_height.store(0, Ordering::Release);
                let total_ms = loader_start.elapsed().as_millis() as u64;
                info!(
                    target: SNAP_STARTUP_TARGET,
                    path = %diag,
                    mode = "empty",
                    total_ms,
                    "snapshot startup: no snapshot row or file"
                );
                info!("snapshot store empty or missing row, fallback to genesis state");
                crate::logger()
                    .info("pwmd startup phase: ready (no snapshot row / file for current backend)");
            }
            Err(e) => {
                let g0_digest = hex::encode(digest(&cfg.state0()));
                let elapsed_ms = loader_start.elapsed().as_millis() as u64;
                warn!(
                    genesis_state0_digest = %g0_digest,
                    err = %e,
                    "snapshot load failed (fallback to genesis state); if state_root mismatch, align genesis params and binary with writer"
                );
                error!(
                    target: SNAP_STARTUP_TARGET,
                    stage = "backend_load",
                    elapsed_ms,
                    err = %e,
                    path = %diag,
                    "snapshot startup degraded"
                );
                let mut st = app.init.write().await;
                *st = InitState::ready_degraded(snap_path.clone(), e.clone());
                crate::logger().error(&format!(
                    "pwmd startup phase: ready_degraded (snapshot error: {e})"
                ));
                shutdown_if_fatal_snapshot(&app, "backend_load");
            }
        }
    });
}

async fn align_summary_post_verify(
    app: &App,
    backend: &SnapshotBackend,
    io_ph: &SnapIoTiming,
    tip_h: u64,
) {
    #[cfg(feature = "clickhouse-snapshot")]
    let (path, io_json) = match (backend, io_ph) {
        (SnapshotBackend::JsonFile { path }, SnapIoTiming::Json(j)) => (path, j),
        _ => return,
    };
    #[cfg(not(feature = "clickhouse-snapshot"))]
    let (path, io_json) = match (backend, io_ph) {
        (SnapshotBackend::JsonFile { path }, SnapIoTiming::Json(j)) => (path, j),
    };
    if !io_json.used_full_verify {
        return;
    }
    let man = match crate::snapshot::incremental::read_epoch_manifest(path.as_path()) {
        Ok(Some(man)) => man,
        Ok(None) => {
            info!("snapshot summary align skipped reason=manifest_missing");
            return;
        }
        Err(err) => {
            warn!("snapshot summary align skipped reason=manifest_read_err err={err}");
            return;
        }
    };
    if man.canonical_h != tip_h {
        info!(
            "snapshot summary align skipped reason=tip_manifest_mismatch tip_h={} manifest_tip={}",
            tip_h, man.canonical_h
        );
        return;
    }
    let persist = {
        let g = app.inner.read().await;
        crate::snapshot::save_checkpoint_summary(path.as_path(), &g)
    };
    match persist {
        Ok(()) => {
            info!(
                "snapshot summary aligned after full_verify checkpoint_height={} reason={}",
                tip_h,
                if io_json.lag_forced_verify {
                    "summary_manifest_lag"
                } else {
                    "verify_chain_flag"
                }
            );
        }
        Err(err) => {
            warn!(
                "snapshot summary align failed checkpoint_height={} err={}",
                tip_h, err
            );
        }
    }
}

pub async fn run_with(config: PwmdConfig) -> Result<(), String> {
    config.validate_persist_snap()?;
    config.validate_cluster_cfg()?;
    debug!("perfmon entities: {}", perfmon::REGISTRY.len());
    if config.transport.enabled && config.transport.peer_listen == config.listen {
        return Err(format!(
            "peer listener must use a dedicated socket; rpc={} peer={}",
            config.listen, config.transport.peer_listen
        ));
    }
    let cors = cors_for_listen(config.listen)?;
    let mut app = app_from_genesis_id(
        &config.genesis,
        config.shard,
        Some(config.data_file.clone()),
        Some(config.identity.clone()),
    )?;
    if app.log_ctl.is_none() {
        app.log_ctl = crate::logging::runtime_log_ctl();
    }
    let bph = {
        let g = app.inner.read().await;
        g.chain.cfg.blocks_per_hour
    };
    let mut cluster_cfg = config.cluster.clone();
    let mut transport_cfg = config.transport.clone();
    let seal_ms = apply_cluster_timing(&mut cluster_cfg, &mut transport_cfg, bph)?;
    if cluster_cfg.seal_ahead_ms > 0 && cluster_cfg.seal_ahead_ms >= seal_ms {
        let clamped = seal_ms.saturating_sub(1).max(1);
        warn!(
            "cluster seal_ahead_ms={} >= seal_interval_ms={}; clamping to {}",
            cluster_cfg.seal_ahead_ms, seal_ms, clamped
        );
        cluster_cfg.seal_ahead_ms = clamped;
    }
    app.op_token = std::env::var("PWM_ADMIN_TOKEN")
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .map(Arc::<str>::from);
    app.rpc_allow = crate::rpc_allow::RpcAllowState::from_cfg(
        &config.rpc_allowed_ips,
        config.rpc_allowed_auto,
    )?;
    if let Some(ref raw) = config.node_instance_id_override {
        let trimmed = raw.trim();
        if !trimmed.is_empty() {
            app.node_instance_id = trimmed.to_string();
            let mut rt = app
                .lease_runtime
                .lock()
                .map_err(|_| "lease runtime mutex poisoned".to_string())?;
            rt.owner_id = trimmed.to_string();
        }
    }
    let stable_id_missing = config
        .node_instance_id_override
        .as_ref()
        .map(|s| s.trim().is_empty())
        .unwrap_or(true);
    if config.cluster.enabled && !config.cluster.members.is_empty() && stable_id_missing {
        warn!(
            "cluster_members are static but --node-instance-id is unset: default wire id is node_id-pid-time_ms and changes each run, so quorum labels will not match --cluster-members across restarts; use --node-instance-id for labs and soak scripts"
        );
    }
    {
        let mut hs = app.handshake.write().await;
        hs.validation_ctx.expected_network_id = config.identity.network_id.clone();
        hs.validation_ctx.expected_genesis_hash = Some({
            let g = app.inner.read().await;
            hex::encode(digest(&g.chain.cfg.state0()))
        });
        hs.local_domain_hi = config.identity.cluster_domain_hi;
    }
    {
        let mut tc = app.transport_config.write().await;
        *tc = transport_cfg.clone();
    }
    {
        let mut st = app.init.write().await;
        *st = InitState::starting(Some(config.data_file.clone()));
    }
    let genesis_d_hex = {
        let g = app.inner.read().await;
        hex::encode(digest(&g.chain.cfg.state0()))
    };
    if let Some(writer) = app.block_writer.take() {
        writer.shutdown()?;
    }
    let backend = config.persisted_snap_backend(&genesis_d_hex)?;
    if let Some(path) = backend.json_path() {
        let g = app.inner.read().await;
        crate::snapshot::incremental::sync_epoch_to_tip(path, &g)?;
    }
    app.block_writer = backend
        .json_path()
        .map(|path| BlockWriter::new(path.to_path_buf()))
        .transpose()?;
    app.autosnapshot_backend = Some(backend);
    app.snapshot_verify_chain = config.snapshot_verify_chain;
    app.exit_on_fatal_snapshot = config.exit_on_fatal_snapshot;
    app.broke_trust_test = config.broke_trust_test;
    app.debug_stop_height = config.debug_stop_height;
    app.debug_dump = config.debug_dump.clone();
    app.debug_disable_seal_loop = config.debug_disable_seal_loop;
    app.lab_seal_api = config.lab_seal_api;
    app.deployment_profile = config.deployment_profile;
    app.seal_role = derive_seal_role(&config);
    {
        let mut manual = app.seal_manual.write().await;
        manual.mode = config.seal_control_mode;
    }
    app.lease_cfg = LeaseCfg {
        ttl_ms: config.seal_lease_ttl_ms,
        takeover_ms: config.seal_takeover_timeout_ms,
        max_tip_lag: config.seal_takeover_tip_lag,
    };
    app.lease_mode = config.seal_lease_backend;
    app.lease_path = match config.seal_lease_backend {
        crate::lease::LeaseBackendMode::File => Some(config.seal_lease_dir.clone()),
        crate::lease::LeaseBackendMode::ProcessLocal => None,
    };
    app.lease_backend = match config.seal_lease_backend {
        crate::lease::LeaseBackendMode::File => Arc::new(
            crate::lease_backend::FileLeaseBackend::open(config.seal_lease_dir.clone())?,
        ),
        crate::lease::LeaseBackendMode::ProcessLocal => {
            Arc::new(crate::lease_backend::ProcessLocalLeaseBackend)
        }
    };
    app.cluster_cfg = cluster_cfg;
    app.block_timing = config.cluster.block_timing_path.as_ref().map(|p| {
        block_timing::BlockTimingCfg::mk_new(
            true,
            p.clone(),
            config.identity.cluster_id.clone(),
            app.node_instance_id.clone(),
            format!("pwmd/{}", env!("CARGO_PKG_VERSION")),
        )
    });
    if let Ok(mut slot) = app.lease_last_err.lock() {
        *slot = None;
    }
    {
        let mut rt = app
            .lease_runtime
            .lock()
            .map_err(|_| "lease runtime mutex poisoned".to_string())?;
        rt.state = if matches!(app.seal_role, SealRole::Active) {
            LeaseState::ActiveSealing
        } else {
            LeaseState::StandbySyncing
        };
    }
    app.debug_align_mid = align_mid_on(config.debug_align_mid, config.debug_det_seal_time);
    {
        let mut hs = app.handshake.write().await;
        hs.deployment_profile = app.deployment_profile;
        hs.local_seal_role = app.seal_role;
        hs.local_validator_hash = Some(app.validator_identity_hash.clone());
        hs.local_instance_id = Some(app.node_instance_id.clone());
    }
    app.dump_count
        .store(0, std::sync::atomic::Ordering::Relaxed);
    {
        let mut g = app.inner.write().await;
        let mode = if config.debug_det_seal_time {
            SealTimeMode::DeterministicHeight
        } else {
            SealTimeMode::WallClock
        };
        g.chain.set_seal_time_mode(mode);
    }
    if app.broke_trust_test {
        warn!(
            "broke_trust_test: peer NodeHello uses a fake genesis digest; handshakes will be rejected by honest nodes (effective_genesis_hash in HTTP status stays canonical)"
        );
    }
    if let Some(stop_h) = app.debug_stop_height {
        warn!(
            "debug-stop-height active (test-only): node will trigger graceful stop at height>={}",
            stop_h
        );
    }
    if config.debug_det_seal_time {
        warn!(
            "debug-deterministic-seal-time active (test/dev-only): seal ts uses deterministic base+height; season/fee time semantics are artificial in this mode"
        );
    }
    if config.debug_align_mid && config.debug_det_seal_time {
        warn!(
            "debug-align-seal-mid-second ignored because debug-deterministic-seal-time is active (deterministic mode wins)"
        );
    } else if app.debug_align_mid {
        warn!(
            "debug-align-seal-mid-second active (test/dev-only): seal loop is aligned near mid-second with bounded wait"
        );
    }
    if config.debug_disable_seal_loop {
        warn!(
            "debug-disable-seal-loop active (test/dev-only): periodic local sealing is disabled; node follows network sync/catch-up only"
        );
    }
    match app.deployment_profile {
        DeploymentProfile::SingleSealer => {
            if matches!(app.lease_mode, crate::lease::LeaseBackendMode::ProcessLocal) {
                warn!(
                    "deployment_profile=single_sealer process-local lease backend is explicitly enabled; same-key multi-process split-brain protection is disabled"
                );
            }
            let lease_path = app
                .lease_path
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "-".to_string());
            info!(
                deployment_profile = "single_sealer",
                seal_role = ?app.seal_role,
                validator_identity_hash = %app.validator_identity_hash,
                node_instance_id = %app.node_instance_id,
                lease_ttl_ms = app.lease_cfg.ttl_ms,
                takeover_timeout_ms = app.lease_cfg.takeover_ms,
                takeover_max_tip_lag = app.lease_cfg.max_tip_lag,
                lease_backend = ?app.lease_mode,
                lease_path = %lease_path,
                "seal deployment profile"
            );
        }
        DeploymentProfile::MultiSealerExperimental => {
            warn!(
                "deployment_profile=multi_sealer_experimental enabled: non-default experimental mode; same-validator active/active protection is relaxed only when explicitly allowed by policy"
            );
            info!(
                deployment_profile = "multi_sealer_experimental",
                seal_role = ?app.seal_role,
                validator_identity_hash = %app.validator_identity_hash,
                node_instance_id = %app.node_instance_id,
                "seal deployment profile"
            );
        }
    }
    if app.cluster_cfg.enabled {
        info!(
            "cluster_attest enabled=true role={:?} members={} quorum={}/{} blocks_per_hour={} seal_interval_ms={} attest_timeout_ms={} heartbeat_interval_ms={} seal_ahead_ms={} note=s2_lease_orthogonal_genesis_timing",
            app.cluster_cfg.role,
            app.cluster_cfg.members.join(","),
            app.cluster_cfg.quorum_k,
            app.cluster_cfg.quorum_n,
            bph,
            seal_ms,
            app.cluster_cfg.attest_timeout_ms,
            transport_cfg.heartbeat_interval_ms,
            app.cluster_cfg.seal_ahead_ms
        );
        info!(
            full_blocks = app.cluster_cfg.full_blocks,
            "cluster propose mode"
        );
    } else {
        info!("cluster_attest enabled=false");
    }
    if config.debug_dump.on_divergence {
        let dir = config
            .debug_dump
            .dir
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "(auto data_file_parent/blocks)".to_string());
        warn!(
            "debug-dump-on-divergence active (debug-only): trigger_streak={} dump_cap={} dump_dir={}",
            config.debug_dump.trigger_streak.max(2),
            config.debug_dump.max_files.max(1),
            dir
        );
    }
    let persist_hint = config.persist_diag_hint();
    info!("pwmd snapshot persist {persist_hint}");
    crate::logger().info(&format!("pwmd snapshot persist {persist_hint}"));
    spawn_snapshot_loader(app.clone());
    spawn_seal_loop(app.clone());
    spawn_federation_sweep_loop(app.clone());
    if transport_cfg.enabled {
        spawn_peer_listener_loop(app.clone(), transport_cfg.clone());
    }
    if transport_cfg.enabled && !transport_cfg.peer_seeds.is_empty() {
        spawn_stateful_transport_loop(app.clone(), transport_cfg.clone());
    } else {
        spawn_transport_loop(app.clone());
    }
    spawn_shutdown_signal_task(app.clone());
    let (shutdown_done_tx, shutdown_done_rx) = tokio::sync::oneshot::channel::<()>();
    {
        let mut slot = app
            .shutdown_tx
            .lock()
            .map_err(|_| "shutdown mutex poisoned".to_string())?;
        *slot = Some(shutdown_done_tx);
    }
    let r = router(app, cors);
    let listener = tokio::net::TcpListener::bind(config.listen)
        .await
        .map_err(|e| format!("bind {}: {e}", config.listen))?;
    let listen_addr = listener
        .local_addr()
        .map_err(|e| format!("local_addr {}: {e}", config.listen))?;
    let mode = runtime_mode_summary(&config.identity.mode);
    let shard_label = runtime_shard_label(&config.identity, config.shard);
    info!(
        "pwmd listen http://{} peer={} shard={} state_ns={} identity=({},0x{:02X},{},{}) mode={}",
        listen_addr,
        config.transport.peer_listen,
        shard_label,
        storage_namespace(&config.identity),
        config.identity.network_id,
        config.identity.cluster_domain_hi,
        config.identity.cluster_id,
        config.identity.node_id,
        mode.as_str()
    );
    crate::logger().info(&format!(
        "pwmd listening on http://{} peer={} shard={} state_ns={} identity=({},0x{:02X},{},{}) mode={}",
        listen_addr,
        config.transport.peer_listen,
        shard_label,
        storage_namespace(&config.identity),
        config.identity.network_id,
        config.identity.cluster_domain_hi,
        config.identity.cluster_id,
        config.identity.node_id,
        mode.as_str()
    ));
    axum::serve(
        listener,
        r.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(async {
        let _ = shutdown_done_rx.await;
        info!("HTTP server graceful shutdown");
    })
    .await
    .map_err(|e| format!("serve: {e}"))?;
    Ok(())
}

pub async fn run() -> Result<(), String> {
    run_with(PwmdConfig::default()).await
}

#[cfg(test)]
mod tests {
    use super::{
        align_next_seal_ms, apply_cluster_timing, attester_alive, autosnap_hit,
        cluster_pending_summary_hit, cluster_prop_ms, cluster_seal_preflight, cluster_timing_ms,
        compute_suppress_pct, count_sync_ready_hs, dedup_seal_entries, gate_recheck_needed,
        is_suppress_alert, lease_renew_log_hit, local_prod_for_h, mk_pick_fatal_diag,
        poll_sleep_ms, record_cluster_prop_tick, run_cluster_gate, run_gate_obs, run_lease_gate,
        seal_drift_adjust_ms, seal_drift_clamp_envelope, seal_drift_in_deadband, seal_envelope_pct,
        seal_interval_ms, should_attempt_seal, should_fire_seal_ahead, skip_evicted_entries,
        skip_missed_h, spawn_seal_loop, tx_bal_diffs, AttSyncCtx, ClusterGateDedup, GateBlock,
        SealAheadWindow, SealPreflight, SealSuppressWindow, SuppressReason,
        AUTOSNAPSHOT_BLOCK_INTERVAL, PROD_PICK_EMPTY_ERR, SEAL_POLL_INTERVAL_MS,
    };
    use crate::bootstrap::app_from_genesis_id;
    use crate::config::GenesisSource;
    use crate::config::TransportConfig;
    use crate::handshake::ClusterRole;
    use crate::handshake::HandshakeValidationCtx;
    use crate::identity::default_runtime_identity_neutral;
    use crate::state::{InitPhase, InitState};
    use crate::transport::{HandshakeState, PeerClass, PeerRecord, PeerStatus, TrustedPeer};
    use crate::ClusterCfg;
    use crate::DevLane;
    use ed25519_dalek::SigningKey;
    use pwm_core::block::hdr_hash;
    use pwm_core::genesis::{FundingCfg, GRow, VRow, ValCfg};
    use pwm_core::hd::{account_id_from_parts, domain_of_account_id};
    use pwm_core::tx::{SignedTx, TxBody};
    use pwm_core::types::Account;
    use pwm_core::{Chain, SealEntry, SealTimeMode};
    use slip10_ed25519::derive_ed25519_private_key;
    use std::collections::HashSet;
    use std::sync::Arc;
    use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

    /// Autosnapshot cadence matches 100-block boundary constant (formerly `autosnapshot_interval_hits_every_100_blocks`).
    #[test]
    fn autosnap_mod100_ok() {
        assert_eq!(AUTOSNAPSHOT_BLOCK_INTERVAL, 100);
        let hits: Vec<u64> = (1..=300).filter(|h| autosnap_hit(*h)).collect();
        assert_eq!(hits, vec![100, 200, 300]);
    }

    #[test]
    fn cluster_pending_summary_decade() {
        let hits: Vec<u64> = (1..=30)
            .filter(|h| cluster_pending_summary_hit(*h))
            .collect();
        assert_eq!(hits, vec![10, 20, 30]);
    }

    fn mk_val(seed: [u8; 32], der_idx: u32) -> (SigningKey, GRow, VRow) {
        let sk = SigningKey::from_bytes(&derive_ed25519_private_key(&seed, &[1_000_000, der_idx]));
        let pk = sk.verifying_key().to_bytes();
        let acct = account_id_from_parts(&pk, der_idx);
        let grow = GRow {
            acct,
            pubkey: pk,
            der_idx,
            bal: 1_000_000,
        };
        let vrow = VRow {
            acct,
            pubkey: pk,
            der_idx,
        };
        (sk, grow, vrow)
    }

    fn seal_tx(nonce: u64, to: u8) -> SignedTx {
        let (gen, sks) = pwm_core::dev_net();
        SignedTx::sign_body(
            &sks[0],
            domain_of_account_id(&gen.accounts[0].acct),
            gen.accounts[0].der_idx,
            nonce,
            TxBody::Transfer {
                to: [to; 32],
                amount: 1,
                fee: 0,
            },
        )
    }

    #[test]
    fn dedup_seal_entries_removes_dup() {
        let tx = seal_tx(0, 7);
        let mut entries = vec![
            SealEntry::Raw(tx.clone()),
            SealEntry::PreValidated { tx, at_height: 4 },
        ];

        dedup_seal_entries(&mut entries);

        assert_eq!(entries.len(), 1);
        assert!(matches!(entries[0], SealEntry::Raw(_)));
    }

    #[test]
    fn eviction_skip_set() {
        let evicted = seal_tx(0, 7);
        let kept = seal_tx(1, 8);
        let kept_hash = kept.tx_hash();
        let hashes = HashSet::from([evicted.tx_hash()]);
        let mut entries = vec![SealEntry::Raw(evicted), SealEntry::Raw(kept)];

        skip_evicted_entries(&mut entries, &hashes);

        assert_eq!(entries.len(), 1);
        assert_eq!(super::seal_entry_hash(&entries[0]), kept_hash);
    }

    #[tokio::test]
    async fn miss_skip_failover_seals() {
        let mut app = app_from_genesis_id(
            &GenesisSource::DevNet,
            DevLane::Lane0,
            None,
            Some(default_runtime_identity_neutral()),
        )
        .expect("app");
        app.cluster_cfg.enabled = true;
        app.cluster_cfg.role = ClusterRole::Proposer;
        app.cluster_cfg.members = vec!["node-b".to_string(), "node-a".to_string()];
        app.cluster_cfg.quorum_n = 2;
        app.cluster_cfg.quorum_k = 1;
        app.node_instance_id = "node-b".to_string();

        let (sk0, grow0, vrow0) = mk_val([41u8; 32], 1);
        let (sk1, grow1, vrow1) = mk_val([42u8; 32], 2);
        let mut cfg = pwm_core::dev_net().0;
        cfg.funding = FundingCfg {
            accounts: vec![grow0, grow1],
        };
        cfg.accounts = cfg.funding.accounts.clone();
        cfg.vals = ValCfg {
            set: vec![vrow0, vrow1],
        };
        cfg.min_validator_stake = 0;

        {
            let mut g = app.inner.write().await;
            g.chain = Chain::boot(cfg, vec![sk0, sk1]);
            g.chain
                .set_seal_time_mode(SealTimeMode::DeterministicHeight);
        }

        assert!(!local_prod_for_h(&app, 1).await.expect("h1 leader"));
        assert!(skip_missed_h(&app, 1).await);
        assert!(local_prod_for_h(&app, 2).await.expect("h2 leader"));
        {
            let mut g = app.inner.write().await;
            g.chain.seal(vec![]).expect("failover seal");
            let blk = g.chain.blocks.back().expect("failover block");
            assert_eq!(blk.hdr.height, 2);
            assert_eq!(blk.hdr.prod_idx, 0);
            assert!(blk.hdr.verify_sig(&g.chain.cfg.vals.set[0].pubkey));
        }
    }

    #[tokio::test]
    async fn prod_pick_fatal_start() {
        let mut app = app_from_genesis_id(
            &GenesisSource::DevNet,
            DevLane::Lane0,
            None,
            Some(default_runtime_identity_neutral()),
        )
        .expect("app");
        app.cluster_cfg.enabled = true;
        app.cluster_cfg.role = ClusterRole::Proposer;
        app.node_instance_id = "node-a".to_string();

        let (sk0, grow0, vrow0) = mk_val([51u8; 32], 1);
        let mut cfg = pwm_core::dev_net().0;
        cfg.funding = FundingCfg {
            accounts: vec![grow0],
        };
        cfg.accounts = cfg.funding.accounts.clone();
        cfg.vals = ValCfg { set: vec![vrow0] };
        cfg.min_validator_stake = 2_000_000;
        {
            let mut g = app.inner.write().await;
            g.chain = Chain::boot(cfg, vec![sk0]);
            g.chain
                .set_seal_time_mode(SealTimeMode::DeterministicHeight);
        }

        let diag = mk_pick_fatal_diag(&app, 1, PROD_PICK_EMPTY_ERR)
            .await
            .expect("fatal diag");
        assert_eq!(diag.tip_h, 0);
        assert_eq!(diag.lead_h, 1);
        assert_eq!(diag.val_n, 1);
        assert_eq!(diag.max_val_stake, 0);
        assert_eq!(diag.min_stake, 2_000_000);
    }

    #[tokio::test]
    async fn epoch_empty_active_midchain_diag() {
        let mut app = app_from_genesis_id(
            &GenesisSource::DevNet,
            DevLane::Lane0,
            None,
            Some(default_runtime_identity_neutral()),
        )
        .expect("app");
        app.cluster_cfg.enabled = true;
        app.cluster_cfg.role = ClusterRole::Proposer;
        app.node_instance_id = "node-a".to_string();

        let (sk0, grow0, vrow0) = mk_val([52u8; 32], 1);
        let mut cfg = pwm_core::dev_net().0;
        cfg.funding = FundingCfg {
            accounts: vec![grow0],
        };
        cfg.accounts = cfg.funding.accounts.clone();
        cfg.vals = ValCfg { set: vec![vrow0] };
        cfg.min_validator_stake = 2_000_000;
        {
            let mut g = app.inner.write().await;
            g.chain = Chain::boot(cfg, vec![sk0]);
            g.chain.set_canon_h(3);
            g.chain.st.active_validator_indices.clear();
            g.chain
                .set_seal_time_mode(SealTimeMode::DeterministicHeight);
        }

        let diag = mk_pick_fatal_diag(&app, 4, PROD_PICK_EMPTY_ERR).await;
        let diag = diag.expect("fatal diag");
        assert_eq!(diag.tip_h, 3);
        assert_eq!(diag.lead_h, 4);
        assert_eq!(diag.val_n, 1);
        assert_eq!(diag.max_val_stake, 0);
        assert_eq!(diag.min_stake, 2_000_000);
    }

    #[test]
    fn lease_renew_log_dedupe() {
        let last = std::sync::atomic::AtomicU64::new(0);
        assert!(!lease_renew_log_hit(&last, 9));
        assert!(lease_renew_log_hit(&last, 10));
        assert!(!lease_renew_log_hit(&last, 10));
        assert!(lease_renew_log_hit(&last, 20));
    }

    #[test]
    fn seal_interval_from_bph() {
        assert_eq!(seal_interval_ms(3600).expect("3600 bph"), 1000);
        assert_eq!(seal_interval_ms(1800).expect("1800 bph"), 2000);
        assert_eq!(seal_interval_ms(3_600_001).expect("high bph"), 1);
        assert!(seal_interval_ms(0).is_err());
    }

    #[test]
    fn cluster_timing_from_bph() {
        let seal_ms = seal_interval_ms(3600).expect("3600 bph");
        assert_eq!(cluster_timing_ms(seal_ms), 2000);
        assert_eq!(cluster_prop_ms(seal_ms, 1500), 1000);
        assert_eq!(cluster_prop_ms(seal_ms, 500), 500);
    }

    /// Cluster enabled + Attester: heartbeat is capped to seal_ms (was 1500ms default).
    #[test]
    fn cluster_apply_attester_hb() {
        let mut cluster = ClusterCfg {
            enabled: true,
            role: ClusterRole::Attester,
            ..ClusterCfg::default()
        };
        let mut transport = TransportConfig::default();
        assert_eq!(transport.heartbeat_interval_ms, 1500);
        let seal_ms =
            apply_cluster_timing(&mut cluster, &mut transport, 3600).expect("apply_cluster_timing");
        assert_eq!(seal_ms, 1000);
        assert_eq!(transport.heartbeat_interval_ms, 1000);
        assert_eq!(cluster.attest_timeout_ms, 2000);
    }

    /// Cluster enabled + Proposer: heartbeat cap behavior unchanged (regression guard).
    #[test]
    fn cluster_apply_proposer_hb() {
        let mut cluster = ClusterCfg {
            enabled: true,
            role: ClusterRole::Proposer,
            ..ClusterCfg::default()
        };
        let mut transport = TransportConfig::default();
        let seal_ms =
            apply_cluster_timing(&mut cluster, &mut transport, 3600).expect("apply_cluster_timing");
        assert_eq!(seal_ms, 1000);
        assert_eq!(transport.heartbeat_interval_ms, 1000);
    }

    /// Cluster disabled: timing fields stay at defaults; heartbeat is not touched.
    #[test]
    fn cluster_apply_disabled_noop() {
        let mut cluster = ClusterCfg::default();
        let mut transport = TransportConfig::default();
        let seal_ms =
            apply_cluster_timing(&mut cluster, &mut transport, 3600).expect("apply_cluster_timing");
        assert_eq!(seal_ms, 1000);
        assert_eq!(transport.heartbeat_interval_ms, 1500);
        assert_eq!(cluster.attest_timeout_ms, 1000);
    }

    /// Configured heartbeat below seal cadence is preserved (min-of semantics).
    #[test]
    fn cluster_apply_keeps_short_hb() {
        let mut cluster = ClusterCfg {
            enabled: true,
            role: ClusterRole::Attester,
            ..ClusterCfg::default()
        };
        let mut transport = TransportConfig {
            heartbeat_interval_ms: 500,
            ..TransportConfig::default()
        };
        let seal_ms =
            apply_cluster_timing(&mut cluster, &mut transport, 3600).expect("apply_cluster_timing");
        assert_eq!(seal_ms, 1000);
        assert_eq!(transport.heartbeat_interval_ms, 500);
    }

    #[test]
    fn drift_slow_decreases_clamped() {
        let (next, ppm) = seal_drift_adjust_ms(1000, 120_000, 100_000);
        assert_eq!(next, 990);
        assert_eq!(ppm, -10_000);
    }

    #[test]
    fn drift_fast_increases_clamped() {
        let (next, ppm) = seal_drift_adjust_ms(1000, 90_000, 100_000);
        assert_eq!(next, 1010);
        assert_eq!(ppm, 10_000);
    }

    #[test]
    fn drift_exact_noop() {
        let (next, ppm) = seal_drift_adjust_ms(1000, 100_000, 100_000);
        assert_eq!(next, 1000);
        assert_eq!(ppm, 0);
    }

    /// Envelope pins windup floor: 257 must clamp to 990 when nominal=1000.
    #[test]
    fn envelope_pins_windup_floor() {
        let (clamped, hit) = seal_drift_clamp_envelope(1000, 257);
        assert_eq!(clamped, 990);
        assert!(hit);
    }

    /// Envelope caps fast-path increases at +1% of nominal.
    #[test]
    fn envelope_caps_fast_increase() {
        let (clamped, hit) = seal_drift_clamp_envelope(1000, 1200);
        assert_eq!(clamped, 1010);
        assert!(hit);
    }

    /// Inside-envelope values are returned untouched with no clamp flag.
    #[test]
    fn envelope_inside_no_clamp() {
        let (clamped, hit) = seal_drift_clamp_envelope(1000, 1003);
        assert_eq!(clamped, 1003);
        assert!(!hit);
        let (clamped, hit) = seal_drift_clamp_envelope(1000, 999);
        assert_eq!(clamped, 999);
        assert!(!hit);
    }

    /// 100 sustained slow windows cannot drive effective below the envelope floor.
    #[test]
    fn envelope_slow_loop_stable() {
        let nominal = 1000u64;
        let mut effective = nominal;
        for _ in 0..100 {
            let actual = 120_000u64;
            let expected = 100_000u64;
            let (next, _ppm) = seal_drift_adjust_ms(effective, actual, expected);
            let (clamped, _hit) = seal_drift_clamp_envelope(nominal, next);
            effective = clamped;
            assert!(
                effective >= 990,
                "effective drifted below floor: {}",
                effective
            );
            assert!(
                effective <= 1010,
                "effective drifted above ceiling: {}",
                effective
            );
        }
        assert_eq!(effective, 990);
    }

    /// 100 sustained fast windows pin effective at the envelope ceiling.
    #[test]
    fn envelope_fast_loop_stable() {
        let nominal = 1000u64;
        let mut effective = nominal;
        for _ in 0..100 {
            let (next, _ppm) = seal_drift_adjust_ms(effective, 90_000, 100_000);
            let (clamped, _hit) = seal_drift_clamp_envelope(nominal, next);
            effective = clamped;
            assert!(effective >= 990);
            assert!(effective <= 1010);
        }
        assert_eq!(effective, 1010);
    }

    /// Deadband skips adjust when actual is within 0.1% of expected.
    #[test]
    fn deadband_skips_tight_drift() {
        assert!(seal_drift_in_deadband(100_050, 100_000));
        assert!(!seal_drift_in_deadband(101_000, 100_000));
        assert!(seal_drift_in_deadband(100_000, 100_000));
    }

    /// Reported envelope_pct stays within ±1.0 after clamp.
    #[test]
    fn envelope_pct_bounded() {
        let (clamped, _) = seal_drift_clamp_envelope(1000, 257);
        let pct = seal_envelope_pct(1000, clamped);
        assert!(
            pct >= -1.0 && pct <= 1.0,
            "envelope_pct out of range: {}",
            pct
        );
        let (clamped, _) = seal_drift_clamp_envelope(1000, 5000);
        let pct = seal_envelope_pct(1000, clamped);
        assert!(
            pct >= -1.0 && pct <= 1.0,
            "envelope_pct out of range: {}",
            pct
        );
    }

    /// Grid alignment lands on the next multiple of nominal_ms for bph=3600.
    #[test]
    fn deadline_grid_bph_3600() {
        assert_eq!(align_next_seal_ms(0, 1000), 1000);
        assert_eq!(align_next_seal_ms(1, 1000), 1000);
        assert_eq!(align_next_seal_ms(999, 1000), 1000);
        assert_eq!(align_next_seal_ms(1000, 1000), 2000);
        assert_eq!(align_next_seal_ms(10_035, 1000), 11_000);
    }

    /// Grid alignment honors arbitrary nominal_ms and guards nominal_ms==0.
    #[test]
    fn deadline_grid_misc_nominal() {
        assert_eq!(align_next_seal_ms(0, 2000), 2000);
        assert_eq!(align_next_seal_ms(2_500, 2000), 4000);
        // nominal_ms=0 falls back to 1ms grid so we never divide by zero.
        assert_eq!(align_next_seal_ms(42, 0), 43);
    }

    /// should_attempt_seal flips exactly at the deadline.
    #[test]
    fn deadline_attempt_edge() {
        assert!(!should_attempt_seal(999, 1000));
        assert!(should_attempt_seal(1000, 1000));
        assert!(should_attempt_seal(1001, 1000));
    }

    #[test]
    fn seal_ahead_trigger_window() {
        assert!(!should_fire_seal_ahead(899, 1000, 100, None));
        assert!(should_fire_seal_ahead(900, 1000, 100, None));
        assert!(should_fire_seal_ahead(999, 1000, 100, None));
        assert!(!should_fire_seal_ahead(1000, 1000, 100, None));
        assert!(!should_fire_seal_ahead(900, 1000, 100, Some(1000)));
        assert!(!should_fire_seal_ahead(900, 1000, 0, None));
    }

    #[test]
    fn seal_ahead_window_avg_lead() {
        let mut win = SealAheadWindow::mk_new();
        assert_eq!(win.avg_lead_ms(), 0);
        win.note_fired(100);
        win.note_fired(80);
        assert_eq!(win.fired, 2);
        assert_eq!(win.avg_lead_ms(), 90);
        win.note_preflight_skip();
        assert_eq!(win.preflight_skip, 1);
        win.reset();
        assert_eq!(win.fired, 0);
    }

    /// Poll sleep is bounded by the deadline gap and never returns zero.
    #[test]
    fn poll_sleep_bounded_gap() {
        // Far from deadline: sleep is capped at SEAL_POLL_INTERVAL_MS.
        assert_eq!(
            poll_sleep_ms(0, 500, SEAL_POLL_INTERVAL_MS),
            SEAL_POLL_INTERVAL_MS
        );
        // Close to deadline: sleep shrinks to the remaining gap.
        assert_eq!(poll_sleep_ms(995, 1000, SEAL_POLL_INTERVAL_MS), 5);
        // Deadline passed: paced retry equal to poll interval.
        assert_eq!(
            poll_sleep_ms(1000, 1000, SEAL_POLL_INTERVAL_MS),
            SEAL_POLL_INTERVAL_MS
        );
        assert_eq!(
            poll_sleep_ms(1500, 1000, SEAL_POLL_INTERVAL_MS),
            SEAL_POLL_INTERVAL_MS
        );
        // poll_ms=0 is clamped to 1.
        assert_eq!(poll_sleep_ms(995, 1000, 0), 1);
    }

    /// Recheck probe is only needed for proposer when first gate check failed.
    #[test]
    fn gate_recheck_policy() {
        assert!(!gate_recheck_needed(false, false));
        assert!(!gate_recheck_needed(true, true));
        assert!(gate_recheck_needed(true, false));
    }

    /// Scheduler invariant: a suppressed iteration must not advance the deadline.
    /// We model the loop by capturing `scheduled_next` only when seal succeeds.
    #[test]
    fn deadline_holds_on_suppress() {
        let nominal = 1000u64;
        let mut deadline = align_next_seal_ms(0, nominal);
        // Suppressed attempt at t=1000: gates close, deadline must stay at 1000.
        let now_a = 1000u64;
        assert!(should_attempt_seal(now_a, deadline));
        // Simulate gate fail -> do NOT update deadline.
        assert_eq!(deadline, 1000);
        // Successful seal at t=1300 advances deadline to next grid boundary.
        let now_b = 1300u64;
        let scheduled = align_next_seal_ms(now_b, nominal);
        deadline = scheduled;
        assert_eq!(deadline, 2000);
    }

    /// compute_suppress_pct returns 0.0 on empty windows (no ticks).
    #[test]
    fn suppress_pct_empty_window() {
        assert_eq!(compute_suppress_pct(0, 0), 0.0);
        assert_eq!(compute_suppress_pct(0, 5), 0.0);
    }

    /// compute_suppress_pct: 93 of 100 ticks → 93.0%.
    #[test]
    fn suppress_pct_typical_share() {
        let pct = compute_suppress_pct(100, 93);
        assert!((pct - 93.0).abs() < 1e-9, "pct={pct}");
    }

    /// is_suppress_alert escalates strictly above the 1.0% owner threshold.
    #[test]
    fn alert_above_one_pct() {
        assert!(!is_suppress_alert(0.0));
        assert!(!is_suppress_alert(1.0));
        assert!(is_suppress_alert(1.01));
        assert!(is_suppress_alert(93.0));
    }

    /// begin_slot counts the deadline once, regardless of how many polls revisit it.
    #[test]
    fn slot_open_idempotent() {
        let mut win = SealSuppressWindow::mk_new();
        let opened1 = win.begin_slot(1000, 10);
        let opened2 = win.begin_slot(1000, 11);
        let opened3 = win.begin_slot(1000, 12);
        assert!(opened1);
        assert!(!opened2);
        assert!(!opened3);
        assert_eq!(win.slots, 1);
        assert_eq!(win.slot_supp, 0);
    }

    /// Many gate-fail polls inside nominal interval do not accrue suppression.
    #[test]
    fn slot_polls_no_inflate() {
        let mut win = SealSuppressWindow::mk_new();
        win.begin_slot(1000, 1_000);
        for now in (1_010..=1_200).step_by(10) {
            assert!(!win.eval_supp(now, 1_000, SuppressReason::ClusterGate));
        }
        assert_eq!(win.slots, 1);
        assert_eq!(win.slot_supp, 0);
        assert!(!win.supp_marked);
    }

    /// After nominal interval elapses, exactly one suppression strike is recorded.
    #[test]
    fn slot_single_strike_after_nominal() {
        let mut win = SealSuppressWindow::mk_new();
        win.begin_slot(1000, 1_000);
        assert!(!win.eval_supp(2_000, 1_000, SuppressReason::ClusterGate));
        assert!(win.eval_supp(2_001, 1_000, SuppressReason::ClusterGate));
        let retry_at = 2_001 + SEAL_POLL_INTERVAL_MS;
        assert!(!win.eval_supp(retry_at, 1_000, SuppressReason::ClusterGate));
        assert_eq!(win.slot_supp, 1);
        assert_eq!(win.last_reason, Some(SuppressReason::ClusterGate));
        assert!(win.supp_marked);
        assert_eq!(win.attempt_start_ms, Some(retry_at));
    }

    /// Seal within the same nominal interval records success without suppression.
    #[test]
    fn slot_seal_inside_nominal() {
        let mut win = SealSuppressWindow::mk_new();
        win.begin_slot(1000, 1_000);
        assert!(!win.eval_supp(1_300, 1_000, SuppressReason::ClusterGate));
        win.close_sealed();
        assert_eq!(win.slots, 1);
        assert_eq!(win.slot_supp, 0);
        assert_eq!(win.sealed_in, 1);
        let pct = compute_suppress_pct(win.slots, win.slot_supp);
        assert_eq!(pct, 0.0);
    }

    /// Sealed close clears suppression for the closed slot.
    #[test]
    fn slot_sealed_clears() {
        let mut win = SealSuppressWindow::mk_new();
        win.begin_slot(1000, 1_000);
        win.eval_supp(2_001, 1_000, SuppressReason::LeaseFence);
        win.close_sealed();
        assert_eq!(win.slots, 1);
        assert_eq!(win.slot_supp, 1);
        assert_eq!(win.sealed_in, 1);
        assert!(win.active_deadline_ms.is_none());
        assert!(win.attempt_start_ms.is_none());
        assert!(!win.supp_marked);
    }

    /// New deadline after one recorded suppression does not add duplicate strike.
    #[test]
    fn slot_skip_no_dup_mark() {
        let mut win = SealSuppressWindow::mk_new();
        win.begin_slot(1000, 1_000);
        assert!(win.eval_supp(2_001, 1_000, SuppressReason::ClusterGate));
        win.begin_slot(2000, 2_010);
        assert_eq!(win.slots, 2);
        assert_eq!(win.slot_supp, 1);
    }

    /// Crossing deadline without prior strike records slot_skipped once.
    #[test]
    fn slot_skip_on_new_grid() {
        let mut win = SealSuppressWindow::mk_new();
        win.begin_slot(1000, 1_000);
        win.begin_slot(2000, 2_000);
        assert_eq!(win.slots, 2);
        assert_eq!(win.slot_supp, 1);
        assert_eq!(win.last_reason, Some(SuppressReason::SlotSkipped));
    }

    /// Window model: 35 sealed in-time, 65 with one strike each.
    #[test]
    fn slot_steady_cy_window() {
        let mut win = SealSuppressWindow::mk_new();
        for i in 0..35u64 {
            let d = (i + 1) * 1000;
            win.begin_slot(d, d);
            win.close_sealed();
        }
        for i in 35..100u64 {
            let d = (i + 1) * 1000;
            win.begin_slot(d, d);
            assert!(win.eval_supp(d + 1001, 1_000, SuppressReason::ClusterGate));
        }
        assert_eq!(win.slots, 100);
        assert_eq!(win.slot_supp, 65);
        assert_eq!(win.sealed_in, 35);
        let pct = compute_suppress_pct(win.slots, win.slot_supp);
        assert!((pct - 65.0).abs() < 1e-9, "pct={pct}");
    }

    /// Wait/timeout counters are split from strike counter and preserved in reset window.
    #[test]
    fn slot_wait_to_split() {
        let mut win = SealSuppressWindow::mk_new();
        win.begin_slot(1000, 1_000);
        assert!(win.note_wait_for_height(42));
        assert!(!win.note_wait_for_height(42));
        assert!(win.note_to_for_height(42));
        assert!(!win.note_to_for_height(42));
        assert!(win.eval_supp(2_001, 1_000, SuppressReason::ClusterGate));
        assert_eq!(win.wait_att, 1);
        assert_eq!(win.gate_to, 1);
        assert_eq!(win.slot_supp, 1);
        win.reset();
        assert_eq!(win.wait_att, 0);
        assert_eq!(win.gate_to, 0);
    }

    /// One WARN-equivalent wait/to/strike per seal target height (no poll inflation).
    #[test]
    fn slot_height_dedup_blocks_repeat() {
        let mut win = SealSuppressWindow::mk_new();
        let h = 33_201u64;
        win.begin_slot(1000, 1_000);
        assert!(win.note_wait_for_height(h));
        assert!(!win.note_wait_for_height(h));
        assert!(win.note_to_for_height(h));
        assert!(!win.note_to_for_height(h));
        assert!(win.eval_supp_for_height(2_001, 1_000, SuppressReason::ClusterGate, Some(h)));
        win.begin_slot(2000, 2_000);
        assert!(!win.eval_supp_for_height(3_002, 1_000, SuppressReason::ClusterGate, Some(h)));
        assert_eq!(win.slot_supp, 1);
    }

    #[test]
    fn cluster_gate_dedup_once_h() {
        let mut d = ClusterGateDedup::default();
        assert!(d.should_log_quorum_timeout(100));
        assert!(!d.should_log_quorum_timeout(100));
        assert!(d.should_log_quorum_wait(100));
        assert!(!d.should_log_quorum_wait(100));
        assert!(d.should_log_quorum_timeout(101));
        d.reset();
        assert!(d.should_log_quorum_timeout(100));
        assert!(d.should_log_quorum_wait(100));
    }

    /// Reset zeros every counter so the next 100s window starts clean.
    #[test]
    fn supp_window_reset_zeros() {
        let mut win = SealSuppressWindow::mk_new();
        win.begin_slot(1000, 1_000);
        win.eval_supp(2_001, 1_000, SuppressReason::LeaseFence);
        win.reset();
        assert_eq!(win.slots, 0);
        assert_eq!(win.slot_supp, 0);
        assert_eq!(win.sealed_in, 0);
        assert!(win.active_deadline_ms.is_none());
        assert!(win.attempt_start_ms.is_none());
        assert!(!win.supp_marked);
        assert!(win.last_reason.is_none());
    }

    /// Preflight: cluster disabled is always Ready (single-node devnet).
    #[test]
    fn preflight_cluster_disabled() {
        let p = cluster_seal_preflight(false, 0, 1);
        assert_eq!(p, SealPreflight::Ready);
    }

    /// Preflight: WaitingAttester when no live attester meets quorum_k.
    #[test]
    fn preflight_waits_no_peer() {
        assert_eq!(
            cluster_seal_preflight(true, 0, 1),
            SealPreflight::WaitingAttester
        );
        assert_eq!(
            cluster_seal_preflight(true, 1, 2),
            SealPreflight::WaitingAttester
        );
    }

    /// Preflight: Ready as soon as live_attesters >= quorum_k.
    #[test]
    fn preflight_ready_on_quorum() {
        assert_eq!(cluster_seal_preflight(true, 1, 1), SealPreflight::Ready);
        assert_eq!(cluster_seal_preflight(true, 3, 2), SealPreflight::Ready);
    }

    #[test]
    fn init_prep_throttle_loading() {
        let init = InitState::loading(None);
        assert_eq!(init.phase, InitPhase::LoadingSnapshot);
        assert_eq!(
            super::init_blocked_reason(init.phase),
            Some("loading_snapshot")
        );
        let now = Instant::now();
        assert!(super::prep_log_due(
            now - Duration::from_secs(super::PREP_SUMMARY_IV_SEC),
            now
        ));
        assert!(!super::prep_log_due(
            now - Duration::from_secs(super::PREP_SUMMARY_IV_SEC - 1),
            now
        ));
    }

    /// Attester liveness: connected + fresh last_seen wins; stale / disconnected fail.
    #[test]
    fn attester_alive_window() {
        let now = 10_000u64;
        let win = 5_000u64;
        assert!(attester_alive(true, 9_000, now, win));
        assert!(attester_alive(true, 5_000, now, win));
        assert!(!attester_alive(true, 4_999, now, win));
        assert!(!attester_alive(false, 9_999, now, win));
        // last_seen_ms == 0 means we never saw a hello: not alive.
        assert!(!attester_alive(true, 0, now, win));
    }

    fn mk_hs() -> HandshakeState {
        HandshakeState::new(
            HandshakeValidationCtx {
                expected_network_id: "devnet".to_string(),
                expected_genesis_hash: None,
                skew_window_ms: 30_000,
            },
            0x10,
        )
    }

    fn mk_trusted(node_id: &str, instance_id: Option<&str>, role: ClusterRole) -> TrustedPeer {
        TrustedPeer {
            node_id: node_id.to_string(),
            cluster_id: "cy".to_string(),
            pubkey: [7u8; 32],
            domain_hi: 0x10,
            instance_id: instance_id.map(str::to_string),
            cluster_attest_enabled: true,
            cluster_role: role,
        }
    }

    fn mk_peer(node_id: &str, status: PeerStatus, last_seen_ms: u64) -> PeerRecord {
        PeerRecord {
            node_id: node_id.to_string(),
            domain_hi: 0x10,
            class: PeerClass::Native,
            last_seen_ms,
            status,
        }
    }

    /// CY mismatch fix: cluster members are instance_id values, not wire node_id keys.
    #[test]
    fn live_attester_count_uses_instance() {
        let mut hs = mk_hs();
        hs.trusted_peers.insert(
            "cy-attester".to_string(),
            mk_trusted(
                "cy-attester",
                Some("cy-quorum-attester"),
                ClusterRole::Attester,
            ),
        );
        hs.peers.insert(
            "cy-attester".to_string(),
            mk_peer("cy-attester", PeerStatus::Connected, 9_900),
        );
        let members = vec![
            "cy-quorum-proposer".to_string(),
            "cy-quorum-attester".to_string(),
        ];
        let out = count_sync_ready_hs(
            &hs,
            &members,
            "cy-quorum-proposer",
            AttSyncCtx {
                now_ms: 10_000,
                local_h: 10,
            },
        );
        assert_eq!(out.live_n, 1);
        assert_eq!(out.sync_n, 1);
    }

    /// Wire-only id without instance_id cannot match cluster member list.
    #[test]
    fn live_attester_no_instance() {
        let mut hs = mk_hs();
        hs.trusted_peers.insert(
            "cy-attester".to_string(),
            mk_trusted("cy-attester", None, ClusterRole::Attester),
        );
        hs.peers.insert(
            "cy-attester".to_string(),
            mk_peer("cy-attester", PeerStatus::Connected, 9_900),
        );
        let members = vec![
            "cy-quorum-proposer".to_string(),
            "cy-quorum-attester".to_string(),
        ];
        let out = count_sync_ready_hs(
            &hs,
            &members,
            "cy-quorum-proposer",
            AttSyncCtx {
                now_ms: 10_000,
                local_h: 10,
            },
        );
        assert_eq!(out.live_n, 0);
        assert_eq!(out.sync_n, 0);
    }

    /// Local member id is excluded even if it appears in trusted peer roster.
    #[test]
    fn live_attester_excludes_local() {
        let mut hs = mk_hs();
        hs.trusted_peers.insert(
            "cy-proposer".to_string(),
            mk_trusted(
                "cy-proposer",
                Some("cy-quorum-proposer"),
                ClusterRole::Attester,
            ),
        );
        hs.peers.insert(
            "cy-proposer".to_string(),
            mk_peer("cy-proposer", PeerStatus::Connected, 9_900),
        );
        let members = vec!["cy-quorum-proposer".to_string()];
        let out = count_sync_ready_hs(
            &hs,
            &members,
            "cy-quorum-proposer",
            AttSyncCtx {
                now_ms: 10_000,
                local_h: 10,
            },
        );
        assert_eq!(out.live_n, 0);
        assert_eq!(out.sync_n, 0);
    }

    /// Liveish statuses count toward preflight, not only Connected.
    #[test]
    fn live_attester_retrying_counts() {
        let mut hs = mk_hs();
        hs.trusted_peers.insert(
            "cy-attester".to_string(),
            mk_trusted(
                "cy-attester",
                Some("cy-quorum-attester"),
                ClusterRole::Attester,
            ),
        );
        hs.peers.insert(
            "cy-attester".to_string(),
            mk_peer("cy-attester", PeerStatus::Retrying, 9_900),
        );
        let members = vec!["cy-quorum-attester".to_string()];
        let out = count_sync_ready_hs(
            &hs,
            &members,
            "cy-quorum-proposer",
            AttSyncCtx {
                now_ms: 10_000,
                local_h: 10,
            },
        );
        assert_eq!(out.live_n, 1);
        assert_eq!(out.sync_n, 1);
    }

    #[test]
    fn sync_ready_lag_huge_no() {
        let mut hs = mk_hs();
        hs.trusted_peers.insert(
            "cy-attester".to_string(),
            mk_trusted(
                "cy-attester",
                Some("cy-quorum-attester"),
                ClusterRole::Attester,
            ),
        );
        hs.peers.insert(
            "cy-attester".to_string(),
            mk_peer("cy-attester", PeerStatus::Connected, 9_900),
        );
        {
            let st = hs
                .sync_live
                .peers
                .entry("cy-attester".to_string())
                .or_default();
            st.tip_h = 33_001;
            st.cup_active = true;
        }
        let members = vec!["cy-quorum-attester".to_string()];
        let out = count_sync_ready_hs(
            &hs,
            &members,
            "cy-quorum-proposer",
            AttSyncCtx {
                now_ms: 10_000,
                local_h: 65_300,
            },
        );
        assert_eq!(out.live_n, 1);
        assert_eq!(out.sync_n, 1);
    }

    #[test]
    fn sync_ready_lag_one_yes() {
        let mut hs = mk_hs();
        hs.trusted_peers.insert(
            "cy-attester".to_string(),
            mk_trusted(
                "cy-attester",
                Some("cy-quorum-attester"),
                ClusterRole::Attester,
            ),
        );
        hs.peers.insert(
            "cy-attester".to_string(),
            mk_peer("cy-attester", PeerStatus::Connected, 9_900),
        );
        {
            let st = hs
                .sync_live
                .peers
                .entry("cy-attester".to_string())
                .or_default();
            st.tip_h = 65_299;
            st.cup_active = false;
        }
        let members = vec!["cy-quorum-attester".to_string()];
        let out = count_sync_ready_hs(
            &hs,
            &members,
            "cy-quorum-proposer",
            AttSyncCtx {
                now_ms: 10_000,
                local_h: 65_300,
            },
        );
        assert_eq!(out.live_n, 1);
        assert_eq!(out.sync_n, 1);
    }

    #[test]
    fn sync_ready_fork_lock_no() {
        let mut hs = mk_hs();
        hs.trusted_peers.insert(
            "cy-attester".to_string(),
            mk_trusted(
                "cy-attester",
                Some("cy-quorum-attester"),
                ClusterRole::Attester,
            ),
        );
        hs.peers.insert(
            "cy-attester".to_string(),
            mk_peer("cy-attester", PeerStatus::Connected, 9_900),
        );
        {
            let st = hs
                .sync_live
                .peers
                .entry("cy-attester".to_string())
                .or_default();
            st.tip_h = 65_299;
            st.fork_h = Some(65_301);
            st.fork_n = 3;
        }
        let members = vec!["cy-quorum-attester".to_string()];
        let out = count_sync_ready_hs(
            &hs,
            &members,
            "cy-quorum-proposer",
            AttSyncCtx {
                now_ms: 10_000,
                local_h: 65_300,
            },
        );
        assert_eq!(out.live_n, 1);
        assert_eq!(out.sync_n, 0);
    }

    /// `tx_bal_diffs` sees both legs of a transfer (formerly `tx_bal_diffs_reports_transfer_changes`).
    #[test]
    fn bal_diff_xfer_two_acct() {
        let (gen, sks) = pwm_core::dev_net();
        let mut before = gen.state0();
        let from = gen.accounts[0].acct;
        let to = [9u8; 32];
        before.accounts.insert(
            to,
            Account {
                initialized: true,
                ..Account::default()
            },
        );
        let mut after = before.clone();
        after.accounts.get_mut(&from).expect("from").balance_pwm -= 12;
        after.accounts.get_mut(&to).expect("to").balance_pwm += 10;
        let tx = SignedTx::sign_body(
            &sks[0],
            domain_of_account_id(&from),
            gen.accounts[0].der_idx,
            0,
            TxBody::Transfer {
                to,
                amount: 10,
                fee: 2,
            },
        );
        let diffs = tx_bal_diffs(&before, &after, &tx);
        assert_eq!(diffs.len(), 2);
    }

    #[test]
    fn seal_skip_ctx_block_h() {
        let (g, sks) = pwm_core::dev_net();
        let mut st = g.state0();
        let signer = g.accounts[0].acct;
        {
            let acc = st.accounts.get_mut(&signer).expect("signer");
            acc.staked_pwm_raw = 2 * pwm_core::display::PWM_RAW_SCALE;
            acc.marks_last_block = 0;
        }
        let tx = SignedTx::sign_body(
            &sks[0],
            domain_of_account_id(&signer),
            g.accounts[0].der_idx,
            0,
            TxBody::Stake { amount: 1 },
        );
        let no_bad = super::first_bad_tx_ctx(&st, &[tx.clone()], 1, 7_200, &g);
        assert!(
            no_bad.is_none(),
            "block-aware ctx should accept a normal stake tx"
        );
        let bad = super::first_bad_tx_ctx(&st, &[tx], 0, 7_200, &g);
        assert!(
            bad.is_none(),
            "stake tx should not depend on block-aware claim context"
        );
    }

    /// Seal loop writes snapshot JSON when data path configured (formerly `seal_writes_snapshot_file_when_data_file_is_configured`).
    #[tokio::test]
    async fn seal_snap_if_datafile() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let snapshot_dir = std::env::temp_dir().join(format!("pwmd-slice19-snapshot-{suffix}"));
        let snapshot_path = snapshot_dir.join("pwm-data.json");
        std::fs::create_dir_all(&snapshot_dir).expect("create snapshot directory");

        let identity = default_runtime_identity_neutral();
        let app = app_from_genesis_id(
            &GenesisSource::DevNet,
            DevLane::Lane0,
            Some(snapshot_path.clone()),
            Some(identity),
        )
        .expect("app boot");

        {
            let mut st = app.init.write().await;
            *st = InitState::ready(Some(snapshot_path.clone()));
        }

        {
            let mut g = app.inner.write().await;
            let (cfg, sks) = pwm_core::dev_net();
            let seed = [99u8; 32];
            let sk1_bytes = derive_ed25519_private_key(&seed, &[0, 1]);
            let sk1 = SigningKey::from_bytes(&sk1_bytes);
            let peer = account_id_from_parts(&sk1.verifying_key().to_bytes(), 1);
            let peer_dom = domain_of_account_id(&peer);
            let init_peer =
                SignedTx::sign_body(&sk1, peer_dom, 1, 0, TxBody::Init { index: 1, flags: 0 });
            g.chain.st.apply_tx(&init_peer).expect("init peer");

            let sender = &sks[0];
            let from = cfg.accounts[0].acct;
            // Prime chain to 99 so first loop seal hits periodic autosnapshot boundary at 100.
            for _ in 0..(AUTOSNAPSHOT_BLOCK_INTERVAL - 1) {
                g.chain.seal(vec![]).expect("prime seal");
                let block = g.chain.blocks.back().expect("prime tip").clone();
                app.block_writer
                    .as_ref()
                    .expect("block writer")
                    .enqueue(Arc::new(block))
                    .expect("enqueue prime block");
            }
            let tx = SignedTx::sign_body(
                sender,
                domain_of_account_id(&from),
                cfg.accounts[0].der_idx,
                0,
                TxBody::Transfer {
                    to: peer,
                    amount: 1,
                    fee: 0,
                },
            );
            g.pool.push(tx).expect("push tx");
        }

        spawn_seal_loop(app.clone());

        // Devnet genesis uses a 1s seal tick; allow first seal attempt before polling.
        tokio::time::sleep(Duration::from_millis(1100)).await;

        let mut ok = false;
        for _ in 0..40 {
            let manifest_path = snapshot_path
                .parent()
                .expect("parent")
                .join("epochs")
                .join("pwm-epochs-manifest.json");
            if manifest_path.exists() {
                ok = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
        assert!(ok, "snapshot file was not created after seal");
        app.shutdown_requested
            .store(true, std::sync::atomic::Ordering::Release);
        let writer = app.block_writer.as_ref().expect("block writer");
        writer.flush().expect("flush block writer");

        let epoch_path = snapshot_dir.join("epochs").join("block_e0.jsonl");
        let epoch = std::fs::read_to_string(&epoch_path).expect("read epoch JSONL");
        let transfer_block = epoch
            .lines()
            .map(|line| serde_json::from_str::<serde_json::Value>(line).expect("parse block JSON"))
            .find(|block| {
                block["txs"]
                    .as_array()
                    .is_some_and(|txs| txs.iter().any(|tx| tx["body"].get("transfer").is_some()))
            })
            .expect("transfer block");
        let transfer = transfer_block["txs"]
            .as_array()
            .expect("block transactions")
            .iter()
            .find(|tx| tx["body"].get("transfer").is_some())
            .expect("transfer transaction");
        let signer_pk = transfer["signer_pk"].as_str().expect("hex signer_pk");
        let signature = transfer["signature"].as_str().expect("hex signature");
        let to = transfer["body"]["transfer"]["to"]
            .as_str()
            .expect("hex transfer.to");
        assert_eq!(signer_pk.len(), 64);
        assert_eq!(signature.len(), 128);
        assert_eq!(to.len(), 64);
        assert!(signer_pk.bytes().all(|byte| byte.is_ascii_hexdigit()));
        assert!(signature.bytes().all(|byte| byte.is_ascii_hexdigit()));
        assert!(to.bytes().all(|byte| byte.is_ascii_hexdigit()));

        writer.shutdown().expect("shutdown block writer");
        std::fs::remove_dir_all(&snapshot_dir).expect("remove snapshot directory");
    }

    #[tokio::test]
    async fn lease_gate_backend_err_closed() {
        use crate::bootstrap::app_from_dev_net;
        use crate::lease::LeaseState;
        use crate::lease_backend::ErrLeaseBackend;

        let mut app = app_from_dev_net();
        app.lease_backend = Arc::new(ErrLeaseBackend {
            msg: "inject_acquire_fail",
        });
        let allow = run_lease_gate(&app).await;
        assert!(!allow);
        let rt = app.lease_runtime.lock().expect("lease mutex");
        assert!(!rt.allow_seal);
        assert!(matches!(rt.state, LeaseState::FencedStandby));
        assert!(
            rt.last_reason.starts_with("lease_backend_error "),
            "reason={}",
            rt.last_reason
        );
        drop(rt);
        let slot = app.lease_last_err.lock().expect("err mutex");
        let got = slot.as_ref().expect("last err");
        assert!(
            got.contains("inject_acquire_fail"),
            "unexpected last err: {got}"
        );
    }

    #[tokio::test]
    async fn cluster_gate_2of2_ok() {
        let mut app = app_from_genesis_id(
            &GenesisSource::DevNet,
            DevLane::Lane0,
            None,
            Some(default_runtime_identity_neutral()),
        )
        .expect("app");
        app.cluster_cfg.enabled = true;
        app.cluster_cfg.role = crate::handshake::ClusterRole::Proposer;
        app.cluster_cfg.members = vec!["node-a".to_string(), "node-b".to_string()];
        app.cluster_cfg.quorum_n = 2;
        app.cluster_cfg.quorum_k = 1;
        let h = app.inner.read().await.chain.tip_h().saturating_add(1);
        {
            let mut hs = app.handshake.write().await;
            let round = hs.cluster_attest.rounds.entry((h, 0)).or_default();
            round.vote_object = "vo1".to_string();
            round.candidate_hash = "aa".repeat(32);
            round.proposer_id = Some("node-a".to_string());
            round.propose_opened_at_ms = Some(crate::current_time_ms().unwrap_or(0));
            round
                .attesters
                .insert("node-b".to_string(), "sig".to_string());
        }
        assert!(run_cluster_gate(&app, None).await);
    }

    #[tokio::test]
    async fn seal_tick_records_prop() {
        let mut app = app_from_genesis_id(
            &GenesisSource::DevNet,
            DevLane::Lane0,
            None,
            Some(default_runtime_identity_neutral()),
        )
        .expect("app");
        app.cluster_cfg.enabled = true;
        app.cluster_cfg.role = crate::handshake::ClusterRole::Proposer;
        app.cluster_cfg.members = vec![app.node_instance_id.clone(), "node-b".to_string()];
        app.cluster_cfg.quorum_n = 2;
        app.cluster_cfg.quorum_k = 1;
        let h = app.inner.read().await.chain.tip_h().saturating_add(1);

        record_cluster_prop_tick(&app).await;

        let hs = crate::transport::handshake_read_traced(&app, "lifecycle").await;
        let round = hs.cluster_attest.rounds.get(&(h, 0)).expect("round");
        assert_eq!(
            round.proposer_id.as_deref(),
            Some(app.node_instance_id.as_str())
        );
        assert!(!round.vote_object.is_empty());
        assert!(!round.candidate_hash.is_empty());
        assert!(round.propose_opened_at_ms.is_none());
    }

    #[tokio::test]
    async fn cluster_gate_reopen_got_zero() {
        let mut app = app_from_genesis_id(
            &GenesisSource::DevNet,
            DevLane::Lane0,
            None,
            Some(default_runtime_identity_neutral()),
        )
        .expect("app");
        app.cluster_cfg.enabled = true;
        app.cluster_cfg.role = crate::handshake::ClusterRole::Proposer;
        app.cluster_cfg.members = vec![
            "node-a".to_string(),
            "node-b".to_string(),
            "node-c".to_string(),
        ];
        app.cluster_cfg.quorum_n = 3;
        app.cluster_cfg.quorum_k = 2;
        app.cluster_cfg.attest_timeout_ms = 10;
        let h = app.inner.read().await.chain.tip_h().saturating_add(1);
        {
            let mut hs = app.handshake.write().await;
            let round = hs.cluster_attest.rounds.entry((h, 0)).or_default();
            round.vote_object = "vo1".to_string();
            round.candidate_hash = "aa".repeat(32);
            round.proposer_id = Some("node-a".to_string());
            round.propose_opened_at_ms = Some(0);
            round.propose_retry_n = 0;
        }
        assert!(!run_cluster_gate(&app, None).await);
        assert!(app
            .cluster_prop_nudge
            .load(std::sync::atomic::Ordering::Acquire));
        let hs = crate::transport::handshake_read_traced(&app, "lifecycle").await;
        let round = hs.cluster_attest.rounds.get(&(h, 0)).expect("round");
        assert_eq!(round.propose_retry_n, 1);
        assert!(round.propose_opened_at_ms.is_none());
    }

    #[tokio::test]
    async fn cluster_gate_2of3_ok() {
        let mut app = app_from_genesis_id(
            &GenesisSource::DevNet,
            DevLane::Lane0,
            None,
            Some(default_runtime_identity_neutral()),
        )
        .expect("app");
        app.cluster_cfg.enabled = true;
        app.cluster_cfg.role = crate::handshake::ClusterRole::Proposer;
        app.cluster_cfg.members = vec![
            "node-a".to_string(),
            "node-b".to_string(),
            "node-c".to_string(),
        ];
        app.cluster_cfg.quorum_n = 3;
        app.cluster_cfg.quorum_k = 1;
        let h = app.inner.read().await.chain.tip_h().saturating_add(1);
        {
            let mut hs = app.handshake.write().await;
            let round = hs.cluster_attest.rounds.entry((h, 0)).or_default();
            round.vote_object = "vo1".to_string();
            round.candidate_hash = "bb".repeat(32);
            round.proposer_id = Some("node-a".to_string());
            round.propose_opened_at_ms = Some(crate::current_time_ms().unwrap_or(0));
            round
                .attesters
                .insert("node-b".to_string(), "sig".to_string());
        }
        assert!(run_cluster_gate(&app, None).await);
    }

    #[tokio::test]
    async fn cluster_gate_2of3_k2_ok() {
        let mut app = app_from_genesis_id(
            &GenesisSource::DevNet,
            DevLane::Lane0,
            None,
            Some(default_runtime_identity_neutral()),
        )
        .expect("app");
        app.cluster_cfg.enabled = true;
        app.cluster_cfg.role = crate::handshake::ClusterRole::Proposer;
        app.cluster_cfg.members = vec![
            "node-a".to_string(),
            "node-b".to_string(),
            "node-c".to_string(),
        ];
        app.cluster_cfg.quorum_n = 3;
        app.cluster_cfg.quorum_k = 2;
        let h = app.inner.read().await.chain.tip_h().saturating_add(1);
        {
            let mut hs = app.handshake.write().await;
            let round = hs.cluster_attest.rounds.entry((h, 0)).or_default();
            round.vote_object = "vo1".to_string();
            round.candidate_hash = "bb".repeat(32);
            round.proposer_id = Some("node-a".to_string());
            round.propose_opened_at_ms = Some(crate::current_time_ms().unwrap_or(0));
            round
                .attesters
                .insert("node-b".to_string(), "sig-b".to_string());
            round
                .attesters
                .insert("node-c".to_string(), "sig-c".to_string());
        }
        assert!(run_cluster_gate(&app, None).await);
    }

    #[tokio::test]
    async fn cluster_gate_quorum_timeout() {
        let mut app = app_from_genesis_id(
            &GenesisSource::DevNet,
            DevLane::Lane0,
            None,
            Some(default_runtime_identity_neutral()),
        )
        .expect("app");
        app.cluster_cfg.enabled = true;
        app.cluster_cfg.role = crate::handshake::ClusterRole::Proposer;
        app.cluster_cfg.members = vec![
            "node-a".to_string(),
            "node-b".to_string(),
            "node-c".to_string(),
        ];
        app.cluster_cfg.quorum_n = 3;
        app.cluster_cfg.quorum_k = 3;
        app.cluster_cfg.attest_timeout_ms = 10;
        let h = app.inner.read().await.chain.tip_h().saturating_add(1);
        {
            let mut hs = app.handshake.write().await;
            let round = hs.cluster_attest.rounds.entry((h, 0)).or_default();
            round.vote_object = "vo1".to_string();
            round.candidate_hash = "aa".repeat(32);
            round.proposer_id = Some("node-a".to_string());
            round.propose_opened_at_ms = Some(
                crate::current_time_ms()
                    .unwrap_or(0)
                    .saturating_sub(app.cluster_cfg.attest_timeout_ms.saturating_add(1)),
            );
            round
                .attesters
                .insert("node-b".to_string(), "sig-b".to_string());
            round
                .attesters
                .insert("node-c".to_string(), "sig-c".to_string());
        }
        assert!(!run_cluster_gate(&app, None).await);

        let obs = run_gate_obs(&app).await;
        assert_eq!(obs, Some(GateBlock::Timeout));
    }

    #[tokio::test]
    async fn seal_loop_disable_no_seal() {
        let identity = default_runtime_identity_neutral();
        let app = app_from_genesis_id(&GenesisSource::DevNet, DevLane::Lane0, None, Some(identity))
            .expect("app");
        {
            let mut st = app.init.write().await;
            *st = InitState::ready(None);
        }
        assert_eq!(app.inner.read().await.chain.tip_h(), 0);
        let mut app = app;
        app.debug_disable_seal_loop = true;
        spawn_seal_loop(app.clone());
        tokio::time::sleep(Duration::from_millis(1100)).await;
        assert_eq!(app.inner.read().await.chain.tip_h(), 0);
    }

    #[tokio::test]
    async fn seal_loop_attester_no_seal() {
        let identity = default_runtime_identity_neutral();
        let app = app_from_genesis_id(&GenesisSource::DevNet, DevLane::Lane0, None, Some(identity))
            .expect("app");
        {
            let mut st = app.init.write().await;
            *st = InitState::ready(None);
        }
        assert_eq!(app.inner.read().await.chain.tip_h(), 0);
        let mut app = app;
        app.cluster_cfg.enabled = true;
        app.cluster_cfg.role = ClusterRole::Attester;
        app.debug_disable_seal_loop = false;
        spawn_seal_loop(app.clone());
        tokio::time::sleep(Duration::from_millis(1100)).await;
        assert_eq!(app.inner.read().await.chain.tip_h(), 0);
    }

    #[tokio::test]
    async fn seal_loop_shutdown_guard() {
        let identity = default_runtime_identity_neutral();
        let app = app_from_genesis_id(&GenesisSource::DevNet, DevLane::Lane0, None, Some(identity))
            .expect("app");
        {
            let mut st = app.init.write().await;
            *st = InitState::ready(None);
        }
        app.shutdown_requested
            .store(true, std::sync::atomic::Ordering::Release);
        spawn_seal_loop(app.clone());
        tokio::time::sleep(Duration::from_millis(200)).await;
        assert_eq!(app.inner.read().await.chain.tip_h(), 0);
    }

    #[test]
    fn derive_role_attester_is_standby() {
        let mut cfg = crate::config::PwmdConfig::default();
        cfg.cluster.enabled = true;
        cfg.cluster.role = ClusterRole::Attester;
        cfg.debug_disable_seal_loop = false;
        assert_eq!(
            super::derive_seal_role(&cfg),
            crate::handshake::SealRole::Standby
        );
    }

    #[tokio::test]
    async fn det_mode_stable_hash_apps() {
        let identity = default_runtime_identity_neutral();
        let app1 =
            app_from_genesis_id(&GenesisSource::DevNet, DevLane::Lane0, None, Some(identity))
                .expect("app1");
        let app2 = app_from_genesis_id(
            &GenesisSource::DevNet,
            DevLane::Lane0,
            None,
            Some(default_runtime_identity_neutral()),
        )
        .expect("app2");
        {
            let mut g1 = app1.inner.write().await;
            let mut g2 = app2.inner.write().await;
            g1.chain
                .set_seal_time_mode(SealTimeMode::DeterministicHeight);
            g2.chain
                .set_seal_time_mode(SealTimeMode::DeterministicHeight);
            g1.chain.seal(vec![]).expect("seal app1");
            g2.chain.seal(vec![]).expect("seal app2");
            let b1 = g1.chain.blocks.back().expect("blk1");
            let b2 = g2.chain.blocks.back().expect("blk2");
            assert_eq!(b1.hdr.ts, b2.hdr.ts);
            assert_eq!(hdr_hash(&b1.hdr), hdr_hash(&b2.hdr));
        }
    }

    #[tokio::test]
    async fn seal_manual_pause_proposer() {
        let mut app = app_from_genesis_id(
            &GenesisSource::DevNet,
            DevLane::Lane0,
            None,
            Some(default_runtime_identity_neutral()),
        )
        .expect("app");
        app.cluster_cfg.enabled = true;
        app.cluster_cfg.role = ClusterRole::Proposer;
        {
            let mut manual = app.seal_manual.write().await;
            manual.mode = crate::SealControlMode::ManualRpc;
        }
        assert!(super::seal_manual_paused(&app).await);
    }

    #[tokio::test]
    async fn seal_manual_pause_auto_noop() {
        let mut app = app_from_genesis_id(
            &GenesisSource::DevNet,
            DevLane::Lane0,
            None,
            Some(default_runtime_identity_neutral()),
        )
        .expect("app");
        app.cluster_cfg.enabled = true;
        app.cluster_cfg.role = ClusterRole::Proposer;
        assert!(!super::seal_manual_paused(&app).await);
    }
}
