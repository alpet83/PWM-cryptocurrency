//! Peer session bookkeeping: errors, disconnect/reconnect counters, wire error classification.

use super::metrics::increment_string_u64_bucket;
use super::{HandshakeState, PeerCloseReason, PeerReconnectReason};
use tracing::{debug, info};

const PEER_TARGET: &str = "pwmd::peer";
const RECONNECT_LOG_ROLLUP_MS: u64 = 10_000;
const HEALTHY_SKIP_LOG_ROLLUP_MS: u64 = 60_000;

pub(crate) fn is_wire_timeout(err: &str) -> bool {
    let lower = err.to_ascii_lowercase();
    lower.contains("timeout") || lower.contains("would block") || lower.contains("wouldblock")
}

pub(crate) fn set_peer_error(hs: &mut HandshakeState, now_ms: u64, msg: impl Into<String>) {
    hs.transport.snapshot.last_peer_error = Some(msg.into());
    hs.transport.snapshot.peer_error_at_ms = Some(now_ms);
}

pub(crate) fn clear_peer_error(hs: &mut HandshakeState) {
    hs.transport.snapshot.last_peer_error = None;
    hs.transport.snapshot.peer_error_at_ms = None;
}

pub(crate) fn record_peer_close(
    hs: &mut HandshakeState,
    now_ms: u64,
    seed: &str,
    node_id: Option<&str>,
    reason: PeerCloseReason,
    detail: Option<&str>,
) {
    let label = reason.as_str();
    let detail_text = detail.unwrap_or("-");
    let sig = format!("{label}:{detail_text}");
    let close_state = hs.close_log.entry(seed.to_string()).or_default();
    if close_state.last_sig == sig {
        close_state.suppressed = close_state.suppressed.saturating_add(1);
        return;
    }
    let repeated = close_state.suppressed;
    close_state.last_sig = sig;
    close_state.suppressed = 0;
    hs.transport.snapshot.last_session_close_reason = Some(label.to_string());
    hs.transport.snapshot.session_disconnected_total = hs
        .transport
        .snapshot
        .session_disconnected_total
        .saturating_add(1);
    increment_string_u64_bucket(
        &mut hs.transport.snapshot.counters.peer_close_by_reason,
        label,
    );
    let node = node_id.unwrap_or("unknown");
    match detail {
        Some(detail) => info!(
            target: PEER_TARGET,
            "peer session close seed={} node_id={} reason={} detail={} repeated={}",
            seed, node, label, detail, repeated
        ),
        None => info!(
            target: PEER_TARGET,
            "peer session close seed={} node_id={} reason={} repeated={}",
            seed, node, label, repeated
        ),
    }
    hs.transport.snapshot.peer_error_at_ms.get_or_insert(now_ms);
}

pub(crate) fn record_reconnect(
    hs: &mut HandshakeState,
    now_ms: u64,
    seed: &str,
    reason: PeerReconnectReason,
    detail: Option<&str>,
) {
    let label = reason.as_str();
    hs.transport.snapshot.last_reconnect_reason = Some(label.to_string());
    increment_string_u64_bucket(
        &mut hs.transport.snapshot.counters.reconnect_decision_by_reason,
        label,
    );
    let state = hs.reconnect_log.entry(seed.to_string()).or_default();
    let rollup_ms = if reason == PeerReconnectReason::HealthySessionSkip {
        HEALTHY_SKIP_LOG_ROLLUP_MS
    } else {
        RECONNECT_LOG_ROLLUP_MS
    };
    if state.last_reason == label && now_for_rollup(state.last_log_ms, now_ms) < rollup_ms {
        state.suppressed = state.suppressed.saturating_add(1);
        return;
    }
    let repeated = state.suppressed;
    state.last_reason = label.to_string();
    state.last_log_ms = now_ms;
    state.suppressed = 0;
    if reason == PeerReconnectReason::HealthySessionSkip {
        debug!(
            target: PEER_TARGET,
            "peer reconnect skipped seed={} reason={} repeated={}",
            seed,
            label,
            repeated
        );
        return;
    }
    match detail {
        Some(detail) => info!(
            target: PEER_TARGET,
            "peer reconnect decision seed={} reason={} detail={} repeated={}",
            seed, label, detail, repeated
        ),
        None => info!(
            target: PEER_TARGET,
            "peer reconnect decision seed={} reason={} repeated={}",
            seed, label, repeated
        ),
    }
}

fn now_for_rollup(last_log_ms: u64, now_ms: u64) -> u64 {
    now_ms.saturating_sub(last_log_ms)
}

pub(crate) fn wire_close_reason(err: &str) -> PeerCloseReason {
    if is_wire_timeout(err) {
        return PeerCloseReason::WireTimeout;
    }
    let lower = err.to_ascii_lowercase();
    if lower.contains("early eof")
        || lower.contains("unexpected eof")
        || lower.contains("connection reset")
        || lower.contains("forcibly closed")
        || lower.contains("broken pipe")
        || lower.contains("connection aborted")
    {
        return PeerCloseReason::Eof;
    }
    PeerCloseReason::ProtocolError
}

pub(crate) fn detail_with_err(label: &str, err: &str) -> String {
    format!("{label}: {err}")
}

pub(crate) fn reconnect_from_close(reason: PeerCloseReason) -> PeerReconnectReason {
    match reason {
        PeerCloseReason::HandshakeFailure | PeerCloseReason::HandshakeRejected => {
            PeerReconnectReason::HandshakeFailure
        }
        PeerCloseReason::WireTimeout => PeerReconnectReason::WireTimeout,
        PeerCloseReason::Eof => PeerReconnectReason::Eof,
        PeerCloseReason::ProtocolError => PeerReconnectReason::ProtocolError,
        PeerCloseReason::ExplicitShutdown => PeerReconnectReason::ExplicitShutdown,
    }
}
