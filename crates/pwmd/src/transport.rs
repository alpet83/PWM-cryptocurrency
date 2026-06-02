//! Transport façade re-exporting dial, handshake state, metrics, and spawn loops.

use super::*;
use crate::handshake::NodeHello;

mod dial;
pub(crate) use dial::attempt_seed_connect;

mod handshake_state;
mod health;
mod lifecycle;
mod metrics;
mod peer_types;
pub use handshake_state::TrustedAccountStreamState;
pub(crate) use handshake_state::{GenesisMismatchSnapshot, HandshakeState};

mod bridges;
mod incoming_hello;
mod policy;
pub(crate) use bridges::{classify_peer_for_hs, transport_outbound_slot};
mod transport_tick;
pub(crate) use incoming_hello::process_incoming_peer_hello;
mod peer_session;
mod spawn;
pub(crate) use peer_session::maybe_retry_round;
pub(crate) use peer_session::record_cluster_prop_tick;
pub(crate) use peer_session::{handshake_read_traced, handshake_write_traced};
pub use spawn::{
    spawn_peer_listener_loop, spawn_real_transport_loop, spawn_stateful_transport_loop,
    spawn_transport_loop,
};

pub(crate) use dial::build_local_node_hello;
pub(crate) use dial::local_hello_signing_key;
#[allow(unused_imports)]
pub(crate) use health::{
    count_native_live_peers, refresh_native_degraded_state, refresh_native_health,
    trusted_relay_count,
};
pub(crate) use lifecycle::{
    clear_peer_error, detail_with_err, is_wire_timeout, reconnect_from_close, record_peer_close,
    record_reconnect, set_peer_error, wire_close_reason,
};
pub use metrics::{ChurnSnapshot, SoakConfidenceSnapshot, TransportCounters, TransportSnapshot};
pub(crate) use metrics::{HandshakeMetrics, TransportPeerState, TransportState};
#[cfg(test)]
use peer_session::{decode_wire_msg_payload, read_wire_msg, write_wire_msg, PeerWireMsg};
use peer_session::{process_inbound_socket, run_seed_session};
pub(crate) use peer_types::DialAttemptResult;
pub(crate) use peer_types::TrustedPeer;
pub use peer_types::{
    BackoffEnvelope, PeerClass, PeerPolicyConfig, PeerPolicyCounters, PeerPolicySnapshot,
    PeerRecord, PeerStatus,
};
use peer_types::{ClassLabel, PeerCloseReason, PeerReconnectReason};
#[allow(unused_imports)]
pub(crate) use transport_tick::run_transport_tick_with;
use transport_tick::{mark_seed_peer_node, set_seed_due};
pub(crate) use transport_tick::{run_real_transport_tick, run_transport_tick};

use metrics::{bounded_add_u64, increment_string_u64_bucket, record_transport_attempt};

#[allow(unused_imports)]
pub(crate) use policy::{
    class_label, classify_peer, dial_attempt_class_key, increment_class_bucket, is_peer_liveish,
    prioritize_peer_candidates, select_backoff_for_class,
};

#[cfg(test)]
mod trust_peer_test;
#[cfg(test)]
pub(crate) use trust_peer_test::trust_peer_for_test;

#[cfg(test)]
mod tests;
