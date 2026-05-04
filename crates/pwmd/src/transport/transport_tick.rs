//! Periodic transport driver: backoff queues, reconnect decisions, dial pacing.

use super::*;
use tracing::warn;

fn enqueue_seed_peer_cls(
    seed: SocketAddr,
    last_peer_class: Option<&PeerClass>,
    native: &mut Vec<SocketAddr>,
    unknown: &mut Vec<SocketAddr>,
    foreign: &mut Vec<SocketAddr>,
) {
    match last_peer_class {
        Some(&PeerClass::Native) => native.push(seed),
        Some(&PeerClass::Foreign) => foreign.push(seed),
        None => unknown.push(seed),
    }
}

fn compute_backoff_delay_ms(base_ms: u64, max_ms: u64, attempts: u32) -> u64 {
    let shift = attempts.saturating_sub(1).min(20);
    let exp = 1u64 << shift;
    base_ms.saturating_mul(exp).min(max_ms)
}

fn backoff_delay_ms(env: &BackoffEnvelope, attempts: u32) -> u64 {
    compute_backoff_delay_ms(env.base_ms, env.max_ms, attempts)
}

fn record_churn_attempt(churn: &mut ChurnSnapshot, result: DialAttemptResult) {
    increment_string_u64_bucket(&mut churn.seed_attempt_by_result, result.as_label());
}

fn apply_transport_peer_result(
    transport_peer: &mut TransportPeerState,
    result: DialAttemptResult,
    now_ms: u64,
    env: &BackoffEnvelope,
) {
    match result {
        DialAttemptResult::Success => {
            transport_peer.attempts = 0;
            transport_peer.next_due_ms = now_ms.saturating_add(env.base_ms);
        }
        DialAttemptResult::RetryableFail => {
            transport_peer.attempts = transport_peer.attempts.saturating_add(1);
            let delay = backoff_delay_ms(env, transport_peer.attempts);
            transport_peer.next_due_ms = now_ms.saturating_add(delay);
        }
    }
}

fn seed_peer_state_mut<'a>(
    hs: &'a mut HandshakeState,
    seed_key: &str,
) -> &'a mut TransportPeerState {
    hs.transport
        .seed_peers
        .entry(seed_key.to_string())
        .or_default()
}

fn rotate_seed_order(peer_seeds: &[SocketAddr], cursor: u64) -> (Vec<SocketAddr>, u64) {
    let seed_count = peer_seeds.len();
    if seed_count == 0 {
        return (Vec::new(), cursor);
    }
    let start = (cursor as usize) % seed_count;
    let mut ordered = Vec::with_capacity(seed_count);
    for i in 0..seed_count {
        ordered.push(peer_seeds[(start + i) % seed_count]);
    }
    (ordered, ((start + 1) % seed_count) as u64)
}

fn update_seed_peer_after_attempt(
    peer_state: &mut TransportPeerState,
    node_id: Option<String>,
    result: DialAttemptResult,
) -> (u32, Option<String>) {
    if let Some(id) = node_id {
        peer_state.last_node_id = Some(id);
    }
    if result == DialAttemptResult::Success {
        peer_state.attempts = 0;
    } else {
        peer_state.attempts = peer_state.attempts.saturating_add(1);
    }
    (peer_state.attempts, peer_state.last_node_id.clone())
}

pub(super) fn set_seed_peer_next_due(hs: &mut HandshakeState, seed_key: &str, next_due_ms: u64) {
    let peer_state = seed_peer_state_mut(hs, seed_key);
    peer_state.next_due_ms = next_due_ms;
}

pub(super) fn mark_seed_peer_node(hs: &mut HandshakeState, seed_key: &str, node_id: &str) {
    let peer_state = seed_peer_state_mut(hs, seed_key);
    peer_state.last_node_id = Some(node_id.to_string());
}

fn soak_counter_cap(cfg: &TransportConfig) -> u64 {
    cfg.soak_counter_cap.max(1)
}

fn refresh_real_tick_state(hs: &mut HandshakeState, cfg: &TransportConfig, now_ms: u64) -> bool {
    let cap = soak_counter_cap(cfg);
    hs.transport.snapshot.ticks_total += 1;
    hs.transport.snapshot.last_tick_attempts = 0;
    hs.transport.snapshot.soak_ticks_capped = hs.transport.snapshot.ticks_total.min(cap);
    if cfg.soak_health_interval_ticks > 0
        && hs.transport.snapshot.ticks_total % cfg.soak_health_interval_ticks == 0
    {
        bounded_add_u64(
            &mut hs.transport.snapshot.soak_health_snapshot_total,
            1,
            cap,
        );
        hs.transport.snapshot.soak_health_last_tick = hs.transport.snapshot.ticks_total;
    }
    if hs.transport.snapshot.reconnect_runaway_guard_active
        && now_ms >= hs.transport.reconnect_runaway_guard_until_ms
    {
        hs.transport.snapshot.reconnect_runaway_guard_active = false;
    }
    hs.transport.snapshot.reconnect_runaway_guard_active
}

fn collect_due_seed_attempts(
    hs: &mut HandshakeState,
    cfg: &TransportConfig,
    now_ms: u64,
) -> Vec<SocketAddr> {
    let (ordered, next_cursor) = rotate_seed_order(
        cfg.peer_seeds.as_slice(),
        hs.transport.snapshot.seed_rotation_cursor,
    );
    hs.transport.snapshot.seed_rotation_cursor = next_cursor;
    hs.churn.seed_rotation_total = hs.churn.seed_rotation_total.saturating_add(1);
    let mut native = Vec::new();
    let mut unknown = Vec::new();
    let mut foreign = Vec::new();
    for seed in ordered {
        let key = seed.to_string();
        let last_node_id = {
            let st = hs.transport.seed_peers.entry(key).or_default();
            if now_ms < st.next_due_ms {
                hs.transport.snapshot.counters.backoff_skip_total += 1;
                continue;
            }
            st.last_node_id.clone()
        };
        let rank = last_node_id
            .as_ref()
            .and_then(|id| hs.peers.get(id))
            .map(|p| &p.class);
        enqueue_seed_peer_cls(seed, rank, &mut native, &mut unknown, &mut foreign);
    }
    let mut due = Vec::with_capacity(native.len() + unknown.len() + foreign.len());
    due.extend(native);
    due.extend(unknown);
    due.extend(foreign);
    let due_len = due.len() as u32;
    let budget = hs
        .transport
        .snapshot
        .tick_attempt_budget
        .max(1)
        .min(due_len) as usize;
    due.truncate(budget);
    hs.transport.snapshot.last_tick_attempts = due.len() as u32;
    due
}

fn apply_reconnect_streak_tick(
    hs: &mut HandshakeState,
    cap: u64,
    tick_retryable: bool,
    tick_success: bool,
) {
    if tick_retryable && !tick_success {
        hs.transport.reconnect_runaway_streak =
            hs.transport.reconnect_runaway_streak.saturating_add(1);
        hs.churn.reconnect_streak_current = hs.transport.reconnect_runaway_streak;
        hs.churn.reconnect_streak_max = hs
            .churn
            .reconnect_streak_max
            .max(hs.churn.reconnect_streak_current);
        bounded_add_u64(&mut hs.churn.unstable_tick_total, 1, cap);
    } else {
        hs.transport.reconnect_runaway_streak = 0;
        hs.churn.reconnect_streak_current = 0;
        bounded_add_u64(&mut hs.churn.stable_tick_total, 1, cap);
    }
}

fn update_known_peer_status(
    hs: &mut HandshakeState,
    last_node_id: &Option<String>,
    status: &PeerStatus,
    now_ms: Option<u64>,
) {
    if let Some(id) = last_node_id {
        if let Some(rec) = hs.peers.get_mut(id) {
            rec.status = status.clone();
            if let Some(ts) = now_ms {
                rec.last_seen_ms = ts;
            }
        }
    }
}

fn apply_seed_attempt_result(
    hs: &mut HandshakeState,
    cfg: &TransportConfig,
    now_ms: u64,
    seed_key: &str,
    result: DialAttemptResult,
    class_key: &str,
    node_id: Option<String>,
) -> (bool, bool) {
    const MAX_RETRY_ATTEMPTS_PER_SEED: u32 = 6;
    let jitter_window = cfg.retry_base_ms.max(50) / 4;
    let jitter = deterministic_seed_jitter_ms(seed_key, now_ms, jitter_window);
    record_transport_attempt(&mut hs.transport.snapshot, class_key, result, now_ms);
    record_churn_attempt(&mut hs.churn, result);
    let (attempts_after, last_node_id) = {
        let peer_state = seed_peer_state_mut(hs, seed_key);
        update_seed_peer_after_attempt(peer_state, node_id, result)
    };
    match result {
        DialAttemptResult::Success => {
            set_seed_peer_next_due(
                hs,
                seed_key,
                now_ms.saturating_add(cfg.retry_base_ms + jitter),
            );
            update_known_peer_status(hs, &last_node_id, &PeerStatus::Connected, Some(now_ms));
            (false, true)
        }
        DialAttemptResult::RetryableFail => {
            update_known_peer_status(hs, &last_node_id, &PeerStatus::Retrying, None);
            if attempts_after >= MAX_RETRY_ATTEMPTS_PER_SEED {
                hs.churn.bounded_retry_cooldowns_total =
                    hs.churn.bounded_retry_cooldowns_total.saturating_add(1);
                hs.churn.disconnected_total = hs.churn.disconnected_total.saturating_add(1);
                update_known_peer_status(hs, &last_node_id, &PeerStatus::Disconnected, None);
                {
                    let peer_state = seed_peer_state_mut(hs, seed_key);
                    peer_state.attempts = 0;
                }
                set_seed_peer_next_due(
                    hs,
                    seed_key,
                    now_ms.saturating_add(cfg.retry_max_ms + jitter),
                );
            } else {
                hs.churn.retrying_total = hs.churn.retrying_total.saturating_add(1);
                let delay = retry_delay_ms(cfg.retry_base_ms, cfg.retry_max_ms, attempts_after);
                set_seed_peer_next_due(hs, seed_key, now_ms.saturating_add(delay + jitter));
            }
            (true, false)
        }
    }
}

fn finalize_real_tick(
    hs: &mut HandshakeState,
    cfg: &TransportConfig,
    now_ms: u64,
    tick_retryable: bool,
    tick_success: bool,
) {
    let cap = soak_counter_cap(cfg);
    apply_reconnect_streak_tick(hs, cap, tick_retryable, tick_success);
    let limit = cfg.reconnect_runaway_streak_limit.max(1);
    if hs.transport.reconnect_runaway_streak >= limit {
        hs.transport.reconnect_runaway_streak = 0;
        hs.churn.reconnect_streak_current = 0;
        hs.transport.snapshot.reconnect_runaway_guard_active = true;
        hs.transport.reconnect_runaway_guard_until_ms =
            now_ms.saturating_add(cfg.reconnect_runaway_cooldown_ms.max(cfg.retry_base_ms));
        warn!(
            target: "pwmd::peer",
            "peer reconnect cooldown entered cooldown_ms={} streak_limit={}",
            cfg.reconnect_runaway_cooldown_ms.max(cfg.retry_base_ms),
            limit
        );
        bounded_add_u64(
            &mut hs.transport.snapshot.reconnect_runaway_stop_total,
            1,
            cap,
        );
    }
}

fn update_seed_health(hs: &mut HandshakeState, cfg: &TransportConfig, now_ms: u64) {
    const WARN_INTERVAL_MS: u64 = 60_000;
    let next_due = hs
        .transport
        .seed_peers
        .values()
        .filter(|p| p.next_due_ms > 0)
        .map(|p| p.next_due_ms)
        .min();
    hs.transport.snapshot.next_seed_due_ms = next_due;
    let relay_live = trusted_relay_count(hs);
    if cfg.peer_seeds.is_empty() || relay_live > 0 {
        return;
    }
    let should_warn = hs
        .transport
        .snapshot
        .last_unhealthy_warn_ms
        .map(|last| now_ms.saturating_sub(last) >= WARN_INTERVAL_MS)
        .unwrap_or(true);
    if !should_warn {
        return;
    }
    hs.transport.snapshot.last_unhealthy_warn_ms = Some(now_ms);
    hs.transport.snapshot.unhealthy_warn_total =
        hs.transport.snapshot.unhealthy_warn_total.saturating_add(1);
    let next_in_ms = next_due.map(|due| due.saturating_sub(now_ms)).unwrap_or(0);
    let last_peer_error = hs
        .transport
        .snapshot
        .last_peer_error
        .as_deref()
        .unwrap_or("none");
    warn!(
        target: "pwmd::peer",
        "peer relay unhealthy: trusted_relay_peer_count=0 seed_count={} next_reconnect_in_ms={} last_peer_error={}",
        cfg.peer_seeds.len(),
        next_in_ms,
        last_peer_error
    );
}

fn select_transport_candidates(hs: &HandshakeState) -> Vec<PeerRecord> {
    prioritize_peer_candidates(hs.local_domain_hi, &hs.peers)
}

pub(crate) fn run_transport_tick_with<F>(hs: &mut HandshakeState, now_ms: u64, mut attempt: F)
where
    F: FnMut(&PeerRecord) -> DialAttemptResult,
{
    hs.transport.snapshot.ticks_total += 1;
    let candidates = select_transport_candidates(hs);
    let mut scheduled_native = 0u32;
    let mut scheduled_foreign = 0u32;
    for peer in candidates {
        let class = classify_peer_for_hs(hs, peer.domain_hi);
        let (scheduled, limit) = transport_outbound_slot(
            &hs.policy,
            &class,
            &mut scheduled_native,
            &mut scheduled_foreign,
        );
        if *scheduled >= limit {
            continue;
        }
        let transport_peer = hs.transport.peers.entry(peer.node_id.clone()).or_default();
        if now_ms < transport_peer.next_due_ms {
            hs.transport.snapshot.counters.backoff_skip_total += 1;
            continue;
        }
        *scheduled += 1;
        let env = select_backoff_for_class(&mut hs.policy, &class);
        let result = attempt(&peer);
        let class_key = class_label(&class);
        record_transport_attempt(&mut hs.transport.snapshot, class_key, result, now_ms);
        apply_transport_peer_result(transport_peer, result, now_ms, &env);
    }
    let native_live = count_native_live_peers(hs);
    refresh_native_health(hs, native_live, true);
}

pub(crate) fn run_transport_tick(hs: &mut HandshakeState, now_ms: u64) {
    run_transport_tick_with(hs, now_ms, |peer| match peer.class {
        PeerClass::Native => DialAttemptResult::Success,
        PeerClass::Foreign => DialAttemptResult::RetryableFail,
    });
}

fn retry_delay_ms(base_ms: u64, max_ms: u64, attempts: u32) -> u64 {
    compute_backoff_delay_ms(base_ms, max_ms, attempts)
}

pub(crate) async fn run_real_transport_tick(app: &App, cfg: &TransportConfig, now_ms: u64) {
    let due;
    {
        let mut hs = app.handshake.write().await;
        let skip_tick = refresh_real_tick_state(&mut hs, cfg, now_ms);
        let seed_count = cfg.peer_seeds.len();
        if seed_count == 0 || skip_tick {
            return;
        }
        due = collect_due_seed_attempts(&mut hs, cfg, now_ms);
    }
    let mut tick_retryable = false;
    let mut tick_success = false;
    for seed in due {
        let (result, class, node_id, peer_err) = attempt_seed_connect(
            app,
            seed,
            cfg.connect_timeout_ms,
            cfg.handshake_timeout_ms,
            now_ms,
        )
        .await;
        let class_key = dial_attempt_class_key(class.as_ref());
        let mut hs = app.handshake.write().await;
        let seed_key = seed.to_string();
        if let Some(err) = peer_err {
            set_peer_error(&mut hs, now_ms, err);
        }
        let (has_retryable, has_success) =
            apply_seed_attempt_result(&mut hs, cfg, now_ms, &seed_key, result, &class_key, node_id);
        tick_retryable |= has_retryable;
        tick_success |= has_success;
    }
    {
        let mut hs = app.handshake.write().await;
        finalize_real_tick(&mut hs, cfg, now_ms, tick_retryable, tick_success);
        update_seed_health(&mut hs, cfg, now_ms);
    }
}

fn deterministic_seed_jitter_ms(seed_key: &str, now_ms: u64, window_ms: u64) -> u64 {
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
