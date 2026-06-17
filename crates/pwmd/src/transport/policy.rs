//! Peer class labels used by dial attempts and handshake acceptance paths.

use super::*;

pub(crate) fn class_label(class: &PeerClass) -> &'static str {
    match class {
        PeerClass::Native => ClassLabel::Native.as_str(),
        PeerClass::Foreign => ClassLabel::Foreign.as_str(),
    }
}

pub(crate) fn dial_attempt_class_key(class: Option<&PeerClass>) -> &'static str {
    class
        .map(class_label)
        .unwrap_or_else(|| ClassLabel::Unknown.as_str())
}

pub(crate) fn classify_peer(local_domain_hi: u8, peer_domain_hi: u8) -> PeerClass {
    if peer_domain_hi == local_domain_hi {
        PeerClass::Native
    } else {
        PeerClass::Foreign
    }
}

pub(super) fn classify_peer_for_hs(hs: &HandshakeState, peer_domain_hi: u8) -> PeerClass {
    classify_peer(hs.local_domain_hi, peer_domain_hi)
}

fn is_native_for_local(local_domain_hi: u8, peer_domain_hi: u8) -> bool {
    peer_domain_hi == local_domain_hi
}

pub(crate) fn is_peer_liveish(status: &PeerStatus) -> bool {
    matches!(
        status,
        PeerStatus::Accepted | PeerStatus::Connected | PeerStatus::Retrying
    )
}

#[allow(dead_code)]
pub(crate) fn prioritize_peer_candidates(
    local_domain_hi: u8,
    peers: &HashMap<String, PeerRecord>,
) -> Vec<PeerRecord> {
    prioritize_peer_candidates_scored(local_domain_hi, peers, &PeerSyncScoreCache::default())
}

pub(crate) fn prioritize_peer_candidates_scored(
    local_domain_hi: u8,
    peers: &HashMap<String, PeerRecord>,
    scores: &PeerSyncScoreCache,
) -> Vec<PeerRecord> {
    fn peer_priority_rank(local_domain_hi: u8, peer_domain_hi: u8) -> u8 {
        if is_native_for_local(local_domain_hi, peer_domain_hi) {
            0
        } else {
            1
        }
    }

    let mut candidates: Vec<PeerRecord> = peers
        .values()
        .filter(|p| is_peer_liveish(&p.status))
        .cloned()
        .collect();
    candidates.sort_by(|a, b| {
        let a_rank = peer_priority_rank(local_domain_hi, a.domain_hi);
        let b_rank = peer_priority_rank(local_domain_hi, b.domain_hi);
        a_rank
            .cmp(&b_rank)
            .then_with(|| scores.get(&b.node_id).cmp(&scores.get(&a.node_id)))
            .then_with(|| b.last_seen_ms.cmp(&a.last_seen_ms))
            .then_with(|| a.node_id.cmp(&b.node_id))
    });
    candidates
}

pub(crate) fn select_backoff_for_class(
    policy: &mut PeerPolicySnapshot,
    class: &PeerClass,
) -> BackoffEnvelope {
    match class {
        PeerClass::Native => {
            policy.counters.backoff_select_native += 1;
            policy.config.native_backoff.clone()
        }
        PeerClass::Foreign => {
            policy.counters.backoff_select_foreign += 1;
            policy.config.foreign_backoff.clone()
        }
    }
}

pub(super) fn transport_outbound_slot<'a>(
    snap: &PeerPolicySnapshot,
    class: &PeerClass,
    scheduled_native: &'a mut u32,
    scheduled_foreign: &'a mut u32,
) -> (&'a mut u32, u32) {
    match class {
        PeerClass::Native => (scheduled_native, snap.config.native_outbound_target),
        PeerClass::Foreign => (scheduled_foreign, snap.config.foreign_outbound_target),
    }
}

pub(crate) fn increment_class_bucket(map: &mut HashMap<String, u64>, class: &PeerClass) {
    super::increment_string_u64_bucket(map, class_label(class));
}
