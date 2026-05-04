//! Transport handshake metrics and periodic dial/backoff counter snapshots.

use super::*;

#[derive(Clone, Debug, Default)]
pub(crate) struct HandshakeMetrics {
    pub(crate) accepted_total: u64,
    pub(crate) rejected_total: u64,
    pub(crate) class_accept_total: HashMap<String, u64>,
    pub(crate) reject_reason_total: HashMap<String, u64>,
}

#[derive(Clone, Debug, Default, Serialize, PartialEq, Eq)]
pub struct TransportCounters {
    pub dial_attempt_by_class_result: HashMap<String, u64>,
    pub peer_close_by_reason: HashMap<String, u64>,
    pub reconnect_decision_by_reason: HashMap<String, u64>,
    pub backoff_skip_total: u64,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct TransportSnapshot {
    pub ticks_total: u64,
    pub counters: TransportCounters,
    pub last_attempt_ms_by_class: HashMap<String, u64>,
    pub last_result_by_class: HashMap<String, String>,
    pub native_underflow_ticks: u64,
    pub native_underflow_threshold_ticks: u64,
    pub native_degraded_state: bool,
    pub native_degraded_transitions: u64,
    pub seed_rotation_cursor: u64,
    pub tick_attempt_budget: u32,
    pub last_tick_attempts: u32,
    pub soak_ticks_capped: u64,
    pub soak_health_snapshot_total: u64,
    pub soak_health_last_tick: u64,
    pub reconnect_runaway_stop_total: u64,
    pub reconnect_runaway_guard_active: bool,
    pub next_seed_due_ms: Option<u64>,
    pub last_unhealthy_warn_ms: Option<u64>,
    pub unhealthy_warn_total: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_peer_error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub peer_error_at_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_session_close_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_reconnect_reason: Option<String>,
    pub session_connected_total: u64,
    pub session_retrying_total: u64,
    pub session_disconnected_total: u64,
    pub session_untrusted_total: u64,
    pub session_trusted_total: u64,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct TransportPeerState {
    pub(crate) attempts: u32,
    pub(crate) next_due_ms: u64,
    pub(crate) last_node_id: Option<String>,
}

#[derive(Clone, Debug)]
pub(crate) struct TransportState {
    pub(crate) peers: HashMap<String, TransportPeerState>,
    pub(crate) seed_peers: HashMap<String, TransportPeerState>,
    pub(crate) snapshot: TransportSnapshot,
    pub(crate) reconnect_runaway_streak: u32,
    pub(crate) reconnect_runaway_guard_until_ms: u64,
}

#[derive(Clone, Debug, Default, Serialize, PartialEq, Eq)]
pub struct ChurnSnapshot {
    pub seed_rotation_total: u64,
    pub retrying_total: u64,
    pub disconnected_total: u64,
    pub bounded_retry_cooldowns_total: u64,
    pub seed_attempt_by_result: HashMap<String, u64>,
    pub reconnect_streak_current: u32,
    pub reconnect_streak_max: u32,
    pub stable_tick_total: u64,
    pub unstable_tick_total: u64,
}

impl Default for TransportState {
    fn default() -> Self {
        Self {
            peers: HashMap::new(),
            seed_peers: HashMap::new(),
            snapshot: TransportSnapshot {
                ticks_total: 0,
                counters: TransportCounters::default(),
                last_attempt_ms_by_class: HashMap::new(),
                last_result_by_class: HashMap::new(),
                native_underflow_ticks: 0,
                native_underflow_threshold_ticks: 3,
                native_degraded_state: false,
                native_degraded_transitions: 0,
                seed_rotation_cursor: 0,
                tick_attempt_budget: 4,
                last_tick_attempts: 0,
                soak_ticks_capped: 0,
                soak_health_snapshot_total: 0,
                soak_health_last_tick: 0,
                reconnect_runaway_stop_total: 0,
                reconnect_runaway_guard_active: false,
                next_seed_due_ms: None,
                last_unhealthy_warn_ms: None,
                unhealthy_warn_total: 0,
                last_peer_error: None,
                peer_error_at_ms: None,
                last_session_close_reason: None,
                last_reconnect_reason: None,
                session_connected_total: 0,
                session_retrying_total: 0,
                session_disconnected_total: 0,
                session_untrusted_total: 0,
                session_trusted_total: 0,
            },
            reconnect_runaway_streak: 0,
            reconnect_runaway_guard_until_ms: 0,
        }
    }
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct SoakConfidenceSnapshot {
    pub loop_ticks_capped: u64,
    pub stable_ticks_capped: u64,
    pub unstable_ticks_capped: u64,
    pub reconnect_streak_current: u32,
    pub reconnect_streak_max: u32,
    pub runaway_stop_total: u64,
    pub runaway_guard_active: bool,
    pub health_snapshot_total: u64,
    pub health_last_tick: u64,
}

pub(super) fn bounded_add_u64(v: &mut u64, delta: u64, cap: u64) {
    if cap == 0 {
        *v = 0;
        return;
    }
    let next = v.saturating_add(delta);
    *v = next.min(cap);
}

fn compose_class_result_key(class_key: &str, result: DialAttemptResult) -> String {
    let result_label = result.as_label();
    let mut key = String::with_capacity(class_key.len() + 1 + result_label.len());
    key.push_str(class_key);
    key.push(':');
    key.push_str(result_label);
    key
}

pub(super) fn increment_string_u64_bucket(map: &mut HashMap<String, u64>, key: &str) {
    if let Some(v) = map.get_mut(key) {
        *v += 1;
        return;
    }
    map.insert(key.to_owned(), 1);
}

pub(super) fn increment_reject_reason_total(metrics: &mut HandshakeMetrics, reason_label: &str) {
    increment_string_u64_bucket(&mut metrics.reject_reason_total, reason_label);
}

pub(super) fn increment_class_accept_total(metrics: &mut HandshakeMetrics, class: &PeerClass) {
    super::increment_class_bucket(&mut metrics.class_accept_total, class);
}

pub(super) fn update_last_attempt_snapshot(
    snapshot: &mut TransportSnapshot,
    class_key: &str,
    now_ms: u64,
    result_label: &str,
) {
    if let Some(ts) = snapshot.last_attempt_ms_by_class.get_mut(class_key) {
        *ts = now_ms;
    } else {
        snapshot
            .last_attempt_ms_by_class
            .insert(class_key.to_owned(), now_ms);
    }
    if let Some(last) = snapshot.last_result_by_class.get_mut(class_key) {
        last.clear();
        last.push_str(result_label);
    } else {
        snapshot
            .last_result_by_class
            .insert(class_key.to_owned(), result_label.to_owned());
    }
}

pub(super) fn record_transport_attempt(
    snapshot: &mut TransportSnapshot,
    class_key: &str,
    result: DialAttemptResult,
    now_ms: u64,
) {
    let key = compose_class_result_key(class_key, result);
    let result_label = result.as_label();
    increment_string_u64_bucket(&mut snapshot.counters.dial_attempt_by_class_result, &key);
    update_last_attempt_snapshot(snapshot, class_key, now_ms, result_label);
}
