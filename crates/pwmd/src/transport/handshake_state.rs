//! Aggregated handshake / transport snapshot state (`App.handshake`).

use std::collections::HashMap;

use serde::Serialize;

use crate::handshake::{HandshakeValidationCtx, ReplayNonceCache};

use super::{
    ChurnSnapshot, HandshakeMetrics, PeerPolicyConfig, PeerPolicyCounters, PeerPolicySnapshot,
    PeerRecord, TransportState, TrustedPeer,
};

#[derive(Clone, Debug)]
pub(crate) struct HandshakeState {
    pub(crate) validation_ctx: HandshakeValidationCtx,
    pub(crate) local_domain_hi: u8,
    pub(crate) replay: ReplayNonceCache,
    pub(crate) peers: HashMap<String, PeerRecord>,
    pub(crate) trusted_peers: HashMap<String, TrustedPeer>,
    pub(crate) trusted_account_streams: HashMap<String, TrustedAccountStreamState>,
    /// Last logged merge `changed` count per peer (throttle repeated INFO lines).
    pub(crate) peer_merge_logged: HashMap<String, usize>,
    pub(crate) metrics: HandshakeMetrics,
    pub(crate) policy: PeerPolicySnapshot,
    pub(crate) transport: TransportState,
    pub(crate) churn: ChurnSnapshot,
    pub(crate) genesis_guard: GenesisGuardState,
    pub(crate) bridge_trust: BridgeTrustState,
    pub(crate) reconnect_log: HashMap<String, ReconnectLogState>,
    pub(crate) close_log: HashMap<String, CloseLogState>,
}

impl HandshakeState {
    pub(crate) fn new(validation_ctx: HandshakeValidationCtx, local_domain_hi: u8) -> Self {
        Self {
            validation_ctx,
            local_domain_hi,
            replay: ReplayNonceCache::default(),
            peers: HashMap::new(),
            trusted_peers: HashMap::new(),
            trusted_account_streams: HashMap::new(),
            peer_merge_logged: HashMap::new(),
            metrics: HandshakeMetrics::default(),
            policy: PeerPolicySnapshot {
                config: PeerPolicyConfig::default(),
                counters: PeerPolicyCounters::default(),
                native_live: 0,
                native_degraded_state: true,
            },
            transport: TransportState::default(),
            churn: ChurnSnapshot::default(),
            genesis_guard: GenesisGuardState::default(),
            bridge_trust: BridgeTrustState::default(),
            reconnect_log: HashMap::new(),
            close_log: HashMap::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct TrustedAccountStreamState {
    pub node_id: String,
    pub domain_hi: u8,
    pub last_update_ms: u64,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct GenesisGuardState {
    pub(crate) blocked: bool,
    pub(crate) mismatch_total: u64,
    pub(crate) last_mismatch: Option<GenesisMismatchSnapshot>,
}

#[derive(Clone, Debug)]
pub(crate) struct GenesisMismatchSnapshot {
    pub(crate) expected_hash: Option<String>,
    pub(crate) received_hash: Option<String>,
    pub(crate) peer_node_id: String,
    pub(crate) peer_hint: String,
    pub(crate) at_unix_ms: u64,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct BridgeTrustState {
    pub(crate) refused: bool,
    pub(crate) refusal_total: u64,
    pub(crate) refusal_reason: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct ReconnectLogState {
    pub(crate) last_reason: String,
    pub(crate) last_log_ms: u64,
    pub(crate) suppressed: u64,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct CloseLogState {
    pub(crate) last_sig: String,
    pub(crate) suppressed: u64,
}
