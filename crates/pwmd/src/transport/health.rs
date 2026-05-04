//! Peer-policy health counters: native live/degraded and trusted relay tallies.

use super::*;

pub(crate) fn refresh_native_degraded_state(policy: &mut PeerPolicySnapshot, native_live: u32) {
    policy.native_live = native_live;
    let next = native_live < policy.config.native_min_live;
    if next != policy.native_degraded_state {
        policy.counters.native_degraded_flips += 1;
        policy.native_degraded_state = next;
    }
}

pub(crate) fn count_native_live_peers(hs: &HandshakeState) -> u32 {
    hs.peers
        .values()
        .filter(|p| super::is_peer_liveish(&p.status) && p.domain_hi == hs.local_domain_hi)
        .count() as u32
}

pub(crate) fn trusted_relay_count(hs: &HandshakeState) -> u32 {
    hs.trusted_peers
        .keys()
        .filter(|node_id| {
            hs.peers
                .get(*node_id)
                .map(|p| super::is_peer_liveish(&p.status))
                .unwrap_or(false)
        })
        .count() as u32
}

pub(crate) fn refresh_native_health(
    hs: &mut HandshakeState,
    native_live: u32,
    with_transport_degraded: bool,
) {
    let under_native_min = native_live < hs.policy.config.native_min_live;
    if with_transport_degraded {
        let snapshot = &mut hs.transport.snapshot;
        if under_native_min {
            snapshot.native_underflow_ticks = snapshot.native_underflow_ticks.saturating_add(1);
        } else {
            snapshot.native_underflow_ticks = 0;
        }
        let next_degraded =
            snapshot.native_underflow_ticks >= snapshot.native_underflow_threshold_ticks;
        if snapshot.native_degraded_state != next_degraded {
            snapshot.native_degraded_state = next_degraded;
            snapshot.native_degraded_transitions =
                snapshot.native_degraded_transitions.saturating_add(1);
        }
    }
    refresh_native_degraded_state(&mut hs.policy, native_live);
}
