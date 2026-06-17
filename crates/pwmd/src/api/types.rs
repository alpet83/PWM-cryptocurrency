//! REST wire types shared by handlers.

use crate::handshake::NodeHello;
use crate::ledger::CrossShardSummary;
use crate::roaming::IntentStatus;
use crate::transport::{
    ChurnSnapshot, PeerClass, PeerPolicySnapshot, PeerRecord, SoakConfidenceSnapshot,
    TransportSnapshot,
};
use pwm_core::SignedTx;
use serde::{Deserialize, Serialize, Serializer};
use std::collections::HashMap;

/// Max JSON body for `POST /v1/tx` (devnet; keeps huge payloads out of the mempool path).
pub const V1_TX_BODY_LIMIT: usize = 256 * 1024;

#[derive(Serialize)]
pub struct StatusOut {
    pub phase: &'static str,
    pub ready: bool,
    pub shard: String,
    pub state_namespace: String,
    pub network_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snapshot_file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snapshot_error: Option<String>,
    pub cluster_domain_hi: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bridge_exported_registry_size: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bridge_imported_set_size: Option<u64>,
    pub bridge_registered_without_import: u64,
    pub cross_shard_summary: CrossShardSummaryOut,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub roaming_intent_pool_size: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub roaming_active_locks_size: Option<u64>,
    pub stuck_exported_without_finalize: u64,
    pub stuck_relayed_without_import: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub oldest_stuck_age_blocks: Option<u64>,
    pub roaming_relay_mode: &'static str,
    pub roaming_relay_hint: String,
    pub peer_seed_count: u64,
    pub peer_listen: String,
    pub live_peer_count: u64,
    pub trusted_relay_peer_count: u64,
    pub peer_session_connected_total: u64,
    pub peer_session_retrying_total: u64,
    pub peer_session_disconnected_total: u64,
    pub peer_session_untrusted_total: u64,
    pub peer_session_trusted_total: u64,
    pub peer_relay_health: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_seed_due_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_peer_error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub peer_error_at_ms: Option<u64>,
    pub genesis_fetch_status: &'static str,
    pub genesis_fetch_hint: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_readiness_reject_code: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_readiness_reject_hint: Option<&'static str>,
    pub balance_semantics: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effective_genesis_hash: Option<String>,
    pub genesis_guard: &'static str,
    pub bridge_federation_trust: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bridge_refusal_reason: Option<String>,
    pub genesis_mismatch_total: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub genesis_mismatch_expected_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub genesis_mismatch_received_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub genesis_mismatch_peer_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub genesis_mismatch_peer_hint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub genesis_mismatch_unix_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub genesis_guard_recovery_hint: Option<&'static str>,
    pub cluster_id: String,
    pub node_id: String,
    pub deployment_profile: String,
    pub seal_role: String,
    pub lease_backend_mode: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lease_backend_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lease_last_backend_error: Option<String>,
    pub validator_identity_hash: String,
    pub node_instance_id: String,
    pub cluster_prep: ClusterPrepOut,
    pub lease_state: String,
    pub seal_gate_allowed: bool,
    pub lease_owner_id: String,
    pub lease_term: u64,
    pub lease_expires_at_ms: u64,
    pub lease_last_tip: u64,
    pub lease_fence: u64,
    pub lease_last_reason: String,
    pub lease_acquire_ok: u64,
    pub lease_renew_ok: u64,
    pub lease_loss_total: u64,
    pub lease_reject_total: u64,
    pub lease_takeover_ok: u64,
}

#[derive(Serialize)]
pub struct ClusterPrepOut {
    pub phase: &'static str,
    pub ready_for_seal: bool,
    pub sync_n: u64,
    pub live_n: u64,
    pub peer_tip_max: u64,
    pub local_tip: u64,
    pub blocks_behind_max: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub waiting_since_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub waiting_sec: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blocked_reason: Option<&'static str>,
}

#[derive(Serialize)]
pub struct CrossShardSummaryOut {
    pub scope: &'static str,
    #[serde(serialize_with = "ser_u128_as_str")]
    pub total_exported_amount: u128,
    pub total_exported_count: u64,
    #[serde(serialize_with = "ser_u128_as_str")]
    pub total_imported_amount: u128,
    pub total_imported_count: u64,
    pub pending_count: u64,
    pub trusted_peer_observed_count: u64,
    pub by_domain_hi: Vec<CrossShardDomainOut>,
}

#[derive(Serialize)]
pub struct CrossShardDomainOut {
    pub domain_hi: u8,
    #[serde(serialize_with = "ser_u128_as_str")]
    pub exported_amount: u128,
    pub exported_count: u64,
    #[serde(serialize_with = "ser_u128_as_str")]
    pub imported_amount: u128,
    pub imported_count: u64,
    pub pending_count: u64,
    pub trusted_peer_observed_count: u64,
}

#[derive(Deserialize)]
pub struct CreateRoamingIntentIn {
    pub tx: SignedTx,
    #[serde(default)]
    pub ttl_blocks: Option<u64>,
}

#[derive(Deserialize)]
pub struct ExportReadinessIn {
    pub tx: SignedTx,
    #[serde(default)]
    pub ttl_sec: Option<u64>,
}

#[derive(Serialize)]
pub struct ExportReadinessOut {
    pub ready: bool,
    pub export_id: String,
    pub expires_at_unix_ms: u64,
    pub reason_code: &'static str,
    pub recovery_hint: &'static str,
}

#[derive(Serialize)]
pub struct CreateRoamingIntentOut {
    pub intent_id: String,
    pub export_id: String,
    pub status: IntentStatus,
    pub created_height: u64,
    pub expires_at_height: u64,
    pub duplicate: bool,
}

#[derive(Serialize)]
pub struct IntentStatusOut {
    pub intent_id: String,
    pub export_id: String,
    pub source: String,
    pub to: String,
    pub target_domain: u16,
    #[serde(serialize_with = "ser_u128_as_str")]
    pub amount: u128,
    #[serde(serialize_with = "ser_u128_as_str")]
    pub fee: u128,
    pub status: IntentStatus,
    pub created_height: u64,
    pub expires_at_height: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
    pub relay_mode: &'static str,
    pub relay_hint: &'static str,
}

#[derive(Serialize)]
pub struct FinalizeRoamingIntentOut {
    pub intent_id: String,
    pub export_id: String,
    pub status: IntentStatus,
    pub changed: bool,
    pub message: String,
    pub handoff: ExportHandoffOut,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExportHandoffOut {
    pub proof_version: u8,
    pub network_id: String,
    pub source_domain_hi: u8,
    pub source_cluster_id: String,
    pub source_node_id: String,
    pub source_node_pubkey: String,
    pub intent_id: String,
    pub export_id: String,
    pub source: String,
    pub to: String,
    pub target_domain: u16,
    pub amount: String,
    pub status: IntentStatus,
    pub signature: String,
}

#[derive(Serialize)]
pub struct RegisterHandoffOut {
    pub export_id: String,
    pub registered: bool,
    pub duplicate: bool,
    pub import_provenance: ImportProvenanceOut,
}

#[derive(Clone, Debug, Deserialize)]
pub struct CrossShardFactsQuery {
    pub target_domain_hi: u8,
    #[serde(default)]
    pub from_height: Option<u64>,
    #[serde(default)]
    pub limit: Option<usize>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CrossShardFactOut {
    pub export_id: String,
    pub source_domain_hi: u8,
    pub target_domain_hi: u8,
    #[serde(deserialize_with = "crate::wire_serde::de_u128_compat")]
    #[serde(serialize_with = "ser_u128_as_str")]
    pub amount: u128,
    pub status: crate::ledger::CrossShardStatus,
    pub first_height: u64,
    pub last_height: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    pub to: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub intent_id: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CrossShardFactsOut {
    pub target_domain_hi: u8,
    pub from_height: u64,
    pub limit: usize,
    pub total: usize,
    pub facts: Vec<CrossShardFactOut>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct BackfillIn {
    #[serde(default)]
    pub from_height: Option<u64>,
    #[serde(default)]
    pub limit: Option<usize>,
    #[serde(default)]
    pub peer_base: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct BackfillOut {
    pub peer: Option<String>,
    pub discovered: u64,
    pub imported: u64,
    pub skipped_existing: u64,
    pub rejected: u64,
    pub untrusted: u64,
    pub details: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ImportProvenanceOut {
    pub to: String,
    pub target_domain: u16,
    #[serde(serialize_with = "ser_u128_as_str")]
    pub amount: u128,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SealControlIn {
    pub mode: crate::SealControlMode,
    #[serde(default)]
    pub verbose_default: Option<bool>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SealStep {
    Preflight,
    Lease,
    Propose,
    GatePoll,
    GateWait,
    SealCommit,
    StepAll,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SealStepIn {
    pub step: SealStep,
    #[serde(default)]
    pub verbose: Option<bool>,
    #[serde(default)]
    pub timeout_ms: Option<u64>,
    #[serde(default)]
    pub target_h: Option<u64>,
}

#[derive(Clone, Debug, Serialize)]
pub struct SealSyncOut {
    pub sync_n: u64,
    pub live_n: u64,
    pub peer_tip_max: u64,
    pub max_lag: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct SealLeaseOut {
    pub state: String,
    pub allow_seal: bool,
    pub owner_id: String,
    pub term: u64,
    pub expires_at_ms: u64,
    pub last_tip: u64,
    pub fence: u64,
    pub reason: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct SealRoundOut {
    pub height: u64,
    pub round: u32,
    pub got: u64,
    pub need: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub propose_opened_at_ms: Option<u64>,
}

#[derive(Clone, Debug, Serialize)]
pub struct SealStatusOut {
    pub mode: crate::SealControlMode,
    pub tip_h: u64,
    pub target_h: u64,
    pub sync_ready: SealSyncOut,
    pub lease: SealLeaseOut,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub round: Option<SealRoundOut>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_step: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_step_ms: Option<u64>,
    pub verbose_active: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct SealGateOut {
    pub got: u64,
    pub need: u8,
    pub elapsed_ms: Option<u64>,
    pub obs: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct SealStepOut {
    pub ok: bool,
    pub step: SealStep,
    pub target_h: u64,
    pub tip_h_after: u64,
    pub duration_ms: u64,
    pub detail: String,
    pub warnings: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gate: Option<SealGateOut>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sync: Option<SealSyncOut>,
}

#[derive(Clone, Debug, Serialize)]
pub struct SealControlOut {
    pub mode: crate::SealControlMode,
    pub verbose_default: bool,
    pub tip_h: u64,
    pub target_h: u64,
    pub verbose_active: bool,
}

#[derive(Serialize)]
pub struct FlowTraceOut {
    pub rows: Vec<crate::state::FlowTraceRow>,
}

pub(super) fn ser_u128_as_str<S>(v: &u128, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(&v.to_string())
}

pub(super) fn cross_shard_summary_out(summary: CrossShardSummary) -> CrossShardSummaryOut {
    CrossShardSummaryOut {
        scope: summary.scope,
        total_exported_amount: summary.total_exported_amount,
        total_exported_count: summary.total_exported_count,
        total_imported_amount: summary.total_imported_amount,
        total_imported_count: summary.total_imported_count,
        pending_count: summary.pending_count,
        trusted_peer_observed_count: summary.trusted_peer_observed_count,
        by_domain_hi: summary
            .by_domain_hi
            .into_iter()
            .map(|row| CrossShardDomainOut {
                domain_hi: row.domain_hi,
                exported_amount: row.exported_amount,
                exported_count: row.exported_count,
                imported_amount: row.imported_amount,
                imported_count: row.imported_count,
                pending_count: row.pending_count,
                trusted_peer_observed_count: row.trusted_peer_observed_count,
            })
            .collect(),
    }
}

#[derive(Serialize)]
pub struct HeadOut {
    pub height: u64,
    pub tip: String,
}

#[derive(Serialize)]
/// V2-2 Slice 0 API freeze: docs/reviews/sprint-v2-2-slice0-account-api-freeze.md.
pub struct AcctOut {
    pub id: String,
    /// Legacy compatibility field.
    /// For foreign accounts this is clamped to `"0"` to avoid
    /// old clients treating local view as spendable truth.
    pub balance_pwm: String,
    pub local_state_balance: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authoritative_home_balance: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authoritative_home_initialized: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub home_lookup_status: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spendable_on_this_shard: Option<String>,
    pub local_view_only: bool,
    pub staked: String,
    pub marks: u32,
    pub marks_last_block: u64,
    pub initialized: bool,
    pub nonce: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rescue_address: Option<String>,
    #[serde(default, skip_serializing_if = "is_zero_u16")]
    pub active_policies: u16,
    #[serde(default, skip_serializing_if = "is_zero_u16")]
    pub dormant_policies: u16,
    #[serde(default, skip_serializing_if = "is_false")]
    pub finalized: bool,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub owner_kind: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub owner_display_name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub owner_country_hint: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub company_metadata_commitment: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub external_verification_ref: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requested_domain_lo: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ipv4_claimed_phase: Option<u8>,
}

#[derive(Serialize)]
pub struct AcctListOut {
    pub accounts: Vec<AcctOut>,
}

fn is_zero_u16(v: &u16) -> bool {
    *v == 0
}

fn is_false(v: &bool) -> bool {
    !*v
}

#[derive(Serialize)]
pub struct PeerHelloOut {
    pub accepted: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub class: Option<PeerClass>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node_hello: Option<NodeHello>,
}

#[derive(Serialize)]
pub struct PeerStatsOut {
    pub accepted_total: u64,
    pub rejected_total: u64,
    pub reject_reason_total: HashMap<String, u64>,
    pub class_accept_total: HashMap<String, u64>,
    pub connected_by_class: HashMap<String, u64>,
    pub peers: Vec<PeerRecord>,
    pub policy: PeerPolicySnapshot,
    pub transport: TransportSnapshot,
    pub churn: ChurnSnapshot,
    pub soak: SoakConfidenceSnapshot,
}
