//! Peer classification enums and handshake-visible peer roster records.

use super::*;
use crate::handshake::ClusterRole;

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PeerClass {
    Native,
    Foreign,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PeerStatus {
    Accepted,
    Rejected,
    Connected,
    Disconnected,
    Retrying,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct PeerRecord {
    pub node_id: String,
    pub domain_hi: u8,
    pub class: PeerClass,
    pub last_seen_ms: u64,
    pub status: PeerStatus,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TrustedPeer {
    pub(crate) node_id: String,
    pub(crate) cluster_id: String,
    pub(crate) pubkey: [u8; 32],
    pub(crate) domain_hi: u8,
    pub(crate) instance_id: Option<String>,
    pub(crate) cluster_attest_enabled: bool,
    pub(crate) cluster_role: ClusterRole,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct BackoffEnvelope {
    pub base_ms: u64,
    pub max_ms: u64,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct PeerPolicyConfig {
    pub native_outbound_target: u32,
    pub foreign_outbound_target: u32,
    pub native_min_live: u32,
    pub native_backoff: BackoffEnvelope,
    pub foreign_backoff: BackoffEnvelope,
    pub class_weights: HashMap<String, u32>,
}

impl Default for PeerPolicyConfig {
    fn default() -> Self {
        let mut class_weights = HashMap::new();
        class_weights.insert(ClassLabel::Native.to_string(), 100);
        class_weights.insert(ClassLabel::Foreign.to_string(), 10);
        Self {
            native_outbound_target: 6,
            foreign_outbound_target: 2,
            native_min_live: 2,
            native_backoff: BackoffEnvelope {
                base_ms: 250,
                max_ms: 4_000,
            },
            foreign_backoff: BackoffEnvelope {
                base_ms: 1_000,
                max_ms: 30_000,
            },
            class_weights,
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, PartialEq, Eq)]
pub struct PeerPolicyCounters {
    pub prioritize_runs: u64,
    pub backoff_select_native: u64,
    pub backoff_select_foreign: u64,
    pub native_degraded_flips: u64,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct PeerPolicySnapshot {
    pub config: PeerPolicyConfig,
    pub counters: PeerPolicyCounters,
    pub native_live: u32,
    pub native_degraded_state: bool,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DialAttemptResult {
    Success,
    RetryableFail,
}

impl DialAttemptResult {
    pub(super) fn as_label(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::RetryableFail => "retryable_fail",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ClassLabel {
    Native,
    Foreign,
    Unknown,
}

impl ClassLabel {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Native => "native",
            Self::Foreign => "foreign",
            Self::Unknown => "unknown",
        }
    }
}

impl std::fmt::Display for ClassLabel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PeerCloseReason {
    HandshakeFailure,
    HandshakeRejected,
    WireTimeout,
    Eof,
    ProtocolError,
    SyncTipDivergence,
    ExplicitShutdown,
}

impl PeerCloseReason {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::HandshakeFailure => "handshake_failure",
            Self::HandshakeRejected => "handshake_rejected",
            Self::WireTimeout => "wire_timeout",
            Self::Eof => "eof",
            Self::ProtocolError => "protocol_error",
            Self::SyncTipDivergence => "sync_tip_divergence",
            Self::ExplicitShutdown => "explicit_shutdown",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PeerReconnectReason {
    RetryAfterClose,
    HealthySessionSkip,
    ConnectFailure,
    HandshakeFailure,
    WireTimeout,
    Eof,
    ProtocolError,
    SyncTipDivergence,
    ExplicitShutdown,
}

impl PeerReconnectReason {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::RetryAfterClose => "retry_after_close",
            Self::HealthySessionSkip => "healthy_session_skip",
            Self::ConnectFailure => "connect_failure",
            Self::HandshakeFailure => "handshake_failure",
            Self::WireTimeout => "wire_timeout",
            Self::Eof => "eof",
            Self::ProtocolError => "protocol_error",
            Self::SyncTipDivergence => "sync_tip_divergence",
            Self::ExplicitShutdown => "explicit_shutdown",
        }
    }
}
