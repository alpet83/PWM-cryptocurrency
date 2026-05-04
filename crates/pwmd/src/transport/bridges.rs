//! Thin forwarding helpers into `policy` (keeps `transport_tick` call sites unchanged).

use super::policy;
use super::{HandshakeState, PeerClass, PeerPolicySnapshot};

pub(crate) fn classify_peer_for_hs(hs: &HandshakeState, peer_domain_hi: u8) -> PeerClass {
    policy::classify_peer_for_hs(hs, peer_domain_hi)
}

pub(crate) fn transport_outbound_slot<'a>(
    snap: &PeerPolicySnapshot,
    class: &PeerClass,
    scheduled_native: &'a mut u32,
    scheduled_foreign: &'a mut u32,
) -> (&'a mut u32, u32) {
    policy::transport_outbound_slot(snap, class, scheduled_native, scheduled_foreign)
}
