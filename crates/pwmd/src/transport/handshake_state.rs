//! Aggregated handshake / transport snapshot state (`App.handshake`).

use std::collections::HashMap;
use std::collections::VecDeque;

use serde::Serialize;

use crate::handshake::{DeploymentProfile, HandshakeValidationCtx, ReplayNonceCache, SealRole};

use super::{
    ChurnSnapshot, HandshakeMetrics, PeerPolicyConfig, PeerPolicyCounters, PeerPolicySnapshot,
    PeerRecord, PeerSyncScoreCache, TransportState, TrustedPeer,
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
    pub(crate) deployment_profile: DeploymentProfile,
    pub(crate) local_seal_role: SealRole,
    pub(crate) local_validator_hash: Option<String>,
    pub(crate) local_instance_id: Option<String>,
    pub(crate) reconnect_log: HashMap<String, ReconnectLogState>,
    pub(crate) close_log: HashMap<String, CloseLogState>,
    pub(crate) mempool_gsp: MempoolGspState,
    pub(crate) sync_live: SyncLiveState,
    pub(crate) cluster_attest: ClusterAttestState,
    pub(crate) peer_scores: PeerSyncScoreCache,
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
            deployment_profile: DeploymentProfile::SingleSealer,
            local_seal_role: SealRole::Active,
            local_validator_hash: None,
            local_instance_id: None,
            reconnect_log: HashMap::new(),
            close_log: HashMap::new(),
            mempool_gsp: MempoolGspState::default(),
            sync_live: SyncLiveState::default(),
            cluster_attest: ClusterAttestState::default(),
            peer_scores: PeerSyncScoreCache::default(),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct MempoolGspState {
    pub(crate) tx_seen_ms: HashMap<String, u64>,
    pub(crate) tx_sent_peer_ms: HashMap<String, HashMap<String, u64>>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct SyncLiveState {
    pub(crate) peers: HashMap<String, SyncPeerState>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct SyncPeerState {
    pub(crate) tip_h: u64,
    pub(crate) tip_hash: Option<String>,
    pub(crate) div_streak: u8,
    pub(crate) wait_hdr_from: Option<u64>,
    pub(crate) wait_hdr_lim: u16,
    pub(crate) wait_blk: VecDeque<(u64, String)>,
    pub(crate) pend_blk: VecDeque<(u64, String)>,
    pub(crate) in_hdr: u8,
    pub(crate) in_blk: u8,
    pub(crate) live_stall: u8,
    pub(crate) cup_active: bool,
    pub(crate) cup_epoch: u64,
    pub(crate) cup_from: u64,
    pub(crate) cup_to: u64,
    pub(crate) cup_next_h: u64,
    pub(crate) cup_next_ix: u32,
    pub(crate) cup_prev_hash: String,
    pub(crate) cup_target_h: u64,
    pub(crate) cup_try: u8,
    pub(crate) cup_next_ms: u64,
    pub(crate) sync_log_ms: u64,
    pub(crate) sync_log_pct: Option<u8>,
    pub(crate) sync_log_done: bool,
    /// Last peer tip goal (height) for which we printed `Sync progress` at 100% / rem=0; avoids per-HB spam.
    pub(crate) sync_pct100_goal: Option<u64>,
    /// Last wall-clock moment when we reported unchanged deep catch-up remainder.
    pub(crate) sync_stall_ms: u64,
    /// Last observed catch-up remainder (`head_h - local_h`) for stall reporting.
    pub(crate) sync_stall_rem: u64,
    /// Last continuity break height detected on live header path.
    pub(crate) fork_h: Option<u64>,
    /// Local tip height at last continuity break.
    pub(crate) fork_tip: u64,
    /// Repeated continuity break count for same `(fork_h, fork_tip)`.
    pub(crate) fork_n: u8,
    /// Local tip hash captured at continuity break.
    pub(crate) fork_local: String,
    /// First remote `prev_hash` captured at continuity break.
    pub(crate) fork_prev: String,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct ClusterAttestState {
    pub(crate) rounds: HashMap<(u64, u32), ClusterRoundState>,
    pub(crate) sent_key_by_node: HashMap<String, (u64, u32)>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct ClusterRoundState {
    pub(crate) vote_object: String,
    pub(crate) candidate_hash: String,
    pub(crate) candidate_ref: Option<String>,
    pub(crate) proposer_id: Option<String>,
    /// Wall-clock ms when the proposer's `ClusterPropose` was accepted (`T_attest` RFC §9 anchor).
    pub(crate) propose_opened_at_ms: Option<u64>,
    /// Bounded proposer-side resend count for rounds with `got=0` attests.
    pub(crate) propose_retry_n: u8,
    pub(crate) attesters: HashMap<String, String>,
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
