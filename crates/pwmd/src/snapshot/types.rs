//! serde snapshot envelope: blocks, accounts state, roaming wire, cross-shard.

use crate::ledger::{CrossShardFact, CrossShardLedger, CrossShardOrigin, CrossShardStatus};
use crate::roaming::{IntentStatus, RoamingIntent, RoamingPool};
use pwm_core::block::{Block, BlockHdr};
use pwm_core::tx::{
    ActivationMode, CosignPolicy, CosignRole, Cosignature, InitPolicyEntry, InitV4Extension,
    PolicyAction, PolicyKind, SignedTx, TxBody,
};
use pwm_core::types::Account;
use pwm_core::State as ChainState;
use serde::Deserialize;
use serde::Serialize;
use serde::{Deserializer, Serializer};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct SnapshotGenesisRow {
    pub(crate) acct: [u8; 32],
    pub(crate) pubkey: [u8; 32],
    pub(crate) der_idx: u32,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub(crate) enum BlocksStored {
    #[default]
    Inline,
    Epochs,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct SnapshotData {
    pub(crate) version: u32,
    pub(crate) genesis_accounts: Vec<SnapshotGenesisRow>,
    pub(crate) blocks: Vec<Block>,
    #[serde(serialize_with = "serialize_snapshot_state")]
    #[serde(deserialize_with = "deserialize_snapshot_state")]
    pub(crate) state: ChainState,
    #[serde(default)]
    pub(crate) roaming: SnapshotRoamingWire,
    #[serde(default)]
    pub(crate) cross_shard: CrossShardLedger,
    /// When [`BlocksStored::Epochs`], `pwm-data.json` omits block bodies (see `epochs/` + manifest).
    #[serde(default)]
    pub(crate) blocks_stored: BlocksStored,
    #[serde(default)]
    pub(crate) checkpoint_height: u64,
}

pub(crate) const SNAPSHOT_VERSION: u32 = 2;
pub(super) const SNAPSHOT_V1: u32 = 1;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct SnapshotDataLegacyV0 {
    pub(super) blocks: Vec<Block>,
    #[serde(serialize_with = "serialize_snapshot_state")]
    #[serde(deserialize_with = "deserialize_snapshot_state")]
    pub(super) state: ChainState,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct SnapshotDataV2 {
    version: u32,
    genesis_accounts: Vec<SnapshotGenesisRowV2>,
    blocks: Vec<BlockV2>,
    state: SnapshotStateV2,
    #[serde(default)]
    roaming: SnapshotRoamingV2,
    #[serde(default)]
    cross_shard: SnapshotCrossShardV2,
    #[serde(default)]
    blocks_stored: BlocksStored,
    #[serde(default)]
    checkpoint_height: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct SnapshotGenesisRowV2 {
    acct: String,
    pubkey: String,
    der_idx: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct BlockV2 {
    hdr: BlockHdrV2,
    txs: Vec<SignedTxV2>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct BlockHdrV2 {
    height: u64,
    prev_hash: String,
    ts: u64,
    prod_idx: u32,
    tx_root: String,
    state_root: String,
    sig: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct SignedTxV2 {
    domain_code: u16,
    signer_pk: String,
    derivation_index: u32,
    nonce: u64,
    body: TxBodyV2,
    #[serde(default)]
    burn_purpose: Option<String>,
    #[serde(default)]
    import_fee: Option<String>,
    #[serde(default)]
    import_provenance: Option<ImportProvV2>,
    #[serde(default)]
    init_v4: Option<InitV4ExtV2>,
    #[serde(default)]
    cosigns: Vec<CosignatureV2>,
    signature: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct CosignatureV2 {
    signer_pk: String,
    role: CosignRole,
    signature: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ImportProvV2 {
    to: String,
    target_domain: u16,
    amount: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct InitV4ExtV2 {
    owner_kind: String,
    owner_display_name: String,
    owner_country_hint: String,
    company_metadata_commitment: String,
    external_verification_ref: String,
    requested_domain_lo: u8,
    #[serde(default)]
    rescue_address: Option<String>,
    #[serde(default)]
    initial_policies: Vec<InitPolicyV2>,
    #[serde(default)]
    cosign_policy: Option<CosignPolicyV2>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct InitPolicyV2 {
    policy: String,
    activation: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct CosignPolicyV2 {
    min_signers: u8,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum TxBodyV2 {
    Init {
        index: u32,
        flags: u32,
    },
    Transfer {
        to: String,
        amount: String,
        fee: String,
    },
    Stake {
        amount: String,
    },
    Unstake {
        amount: String,
    },
    BurnMark {
        mark_amount: String,
        beneficiary: Option<String>,
    },
    Claim {
        mode: String,
        claim_units: String,
        anchor_ref: u64,
        fee: String,
    },
    Export {
        to: String,
        target_domain: u16,
        amount: String,
        fee: String,
    },
    Import {
        to: String,
        amount: String,
        export_id: String,
    },
    Policy {
        target_account: String,
        action: PolicyActionV2,
        fee: String,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum PolicyActionV2 {
    SetPolicy { policy: String, activation: String },
    ActivatePolicy { policy_id: u8 },
    DeactivatePolicy { policy_id: u8 },
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct SnapshotRoamingV2 {
    #[serde(default)]
    intents: Vec<SnapshotIntentRowV2>,
    #[serde(default)]
    locks: Vec<SnapshotIntentLockRowV2>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct SnapshotIntentRowV2 {
    intent_id: String,
    export_id: String,
    source: String,
    to: String,
    target_domain: u16,
    amount: String,
    fee: String,
    status: IntentStatus,
    created_height: u64,
    expires_at_height: u64,
    last_error: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct SnapshotIntentLockRowV2 {
    source: String,
    intent_id: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct SnapshotCrossShardV2 {
    #[serde(default)]
    facts: Vec<SnapshotCrossShardFactV2>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct SnapshotCrossShardFactV2 {
    export_id: String,
    source_domain_hi: u8,
    target_domain_hi: u8,
    amount: String,
    status: CrossShardStatus,
    first_height: u64,
    last_height: u64,
    source: Option<String>,
    to: String,
    intent_id: Option<String>,
    #[serde(default)]
    origin: CrossShardOrigin,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct SnapshotStateV2 {
    accounts: Vec<SnapshotStateRowV2>,
    fee_pool: String,
    // Legacy mirror field: accepted on read for compatibility, omitted on write.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    marks_quota: Vec<SnapshotQuotaRowV2>,
    #[serde(default)]
    imported_set: Vec<String>,
    #[serde(default)]
    exported_registry: Vec<SnapshotExportRowV2>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct SnapshotStateRowV2 {
    id: String,
    account: SnapshotAccountV2,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct SnapshotQuotaRowV2 {
    id: String,
    quota: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct SnapshotAccountV2 {
    signing_pubkey: String,
    derivation_index: u32,
    balance_pwm: String,
    staked: String,
    marks: String,
    initialized: bool,
    index: u32,
    flags: u32,
    nonce: u64,
    #[serde(default)]
    last_claim_unix_time: u64,
    #[serde(default)]
    last_claim_anchor_ref: u64,
    #[serde(default, rename = "last_free_claim_utc_day")]
    free_claim_utc_day: Option<u64>,
    #[serde(default)]
    last_stake_change_height: u64,
    #[serde(default)]
    rescue_address: Option<String>,
    #[serde(default)]
    active_policies: u16,
    #[serde(default)]
    dormant_policies: u16,
    #[serde(default)]
    finalized: bool,
    #[serde(default)]
    owner_kind: String,
    #[serde(default)]
    owner_display_name: String,
    #[serde(default)]
    owner_country_hint: String,
    #[serde(default)]
    company_metadata_commitment: Option<String>,
    #[serde(default)]
    external_verification_ref: Option<String>,
    #[serde(default)]
    requested_domain_lo: Option<u8>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct SnapshotExportRowV2 {
    export_id: String,
    to: String,
    target_domain: u16,
    amount: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct SnapshotRoamingWire {
    #[serde(default)]
    intents: Vec<SnapshotIntentRow>,
    #[serde(default)]
    locks: Vec<SnapshotIntentLockRow>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct SnapshotIntentRow {
    intent_id: [u8; 32],
    export_id: [u8; 32],
    source: [u8; 32],
    to: [u8; 32],
    target_domain: u16,
    amount: u128,
    fee: u128,
    status: IntentStatus,
    created_height: u64,
    expires_at_height: u64,
    last_error: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct SnapshotIntentLockRow {
    source: [u8; 32],
    intent_id: [u8; 32],
}

pub(crate) fn roaming_to_wire(pool: &RoamingPool) -> SnapshotRoamingWire {
    SnapshotRoamingWire {
        intents: pool
            .intents_snapshot()
            .into_iter()
            .map(|intent| SnapshotIntentRow {
                intent_id: intent.intent_id,
                export_id: intent.export_id,
                source: intent.source,
                to: intent.to,
                target_domain: intent.target_domain,
                amount: intent.amount,
                fee: intent.fee,
                status: intent.status,
                created_height: intent.created_height,
                expires_at_height: intent.expires_at_height,
                last_error: intent.last_error,
            })
            .collect(),
        locks: pool
            .active_locks_snapshot()
            .into_iter()
            .map(|(source, intent_id)| SnapshotIntentLockRow { source, intent_id })
            .collect(),
    }
}

fn roaming_from_wire(wire: SnapshotRoamingWire) -> Result<RoamingPool, String> {
    let intents = wire
        .intents
        .into_iter()
        .map(|row| RoamingIntent {
            intent_id: row.intent_id,
            export_id: row.export_id,
            source: row.source,
            to: row.to,
            target_domain: row.target_domain,
            amount: row.amount,
            fee: row.fee,
            status: row.status,
            created_height: row.created_height,
            expires_at_height: row.expires_at_height,
            last_error: row.last_error,
        })
        .collect();
    let locks = wire
        .locks
        .into_iter()
        .map(|row| (row.source, row.intent_id))
        .collect();
    RoamingPool::restore_from_snapshot(intents, locks)
}

fn cross_shard_to_v2(value: &CrossShardLedger) -> SnapshotCrossShardV2 {
    SnapshotCrossShardV2 {
        facts: value
            .facts()
            .into_iter()
            .map(|fact| SnapshotCrossShardFactV2 {
                export_id: hex_of(&fact.export_id),
                source_domain_hi: fact.source_domain_hi,
                target_domain_hi: fact.target_domain_hi,
                amount: dec_of(fact.amount),
                status: fact.status,
                first_height: fact.first_height,
                last_height: fact.last_height,
                source: fact.source.as_ref().map(hex_of),
                to: hex_of(&fact.to),
                intent_id: fact.intent_id.as_ref().map(hex_of),
                origin: fact.origin,
            })
            .collect(),
    }
}

fn cross_shard_from_v2(value: SnapshotCrossShardV2) -> Result<CrossShardLedger, String> {
    let mut ledger = CrossShardLedger::default();
    for (i, row) in value.facts.into_iter().enumerate() {
        let export_id = hex_v2(&row.export_id, &format!("cross_shard.facts[{i}].export_id"))?;
        let source = row
            .source
            .as_deref()
            .map(|v| hex_v2(v, &format!("cross_shard.facts[{i}].source")))
            .transpose()?;
        let intent_id = row
            .intent_id
            .as_deref()
            .map(|v| hex_v2(v, &format!("cross_shard.facts[{i}].intent_id")))
            .transpose()?;
        let fact = CrossShardFact {
            export_id,
            source_domain_hi: row.source_domain_hi,
            target_domain_hi: row.target_domain_hi,
            amount: dec_v2(&row.amount, &format!("cross_shard.facts[{i}].amount"))?,
            status: row.status,
            first_height: row.first_height,
            last_height: row.last_height,
            source,
            to: hex_v2(&row.to, &format!("cross_shard.facts[{i}].to"))?,
            intent_id,
            origin: row.origin,
        };
        ledger.insert_fact(fact);
    }
    Ok(ledger)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct SnapshotStateRow {
    id: [u8; 32],
    account: SnapshotAccountWire,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct SnapshotQuotaRow {
    id: [u8; 32],
    quota: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct SnapshotStateWire {
    accounts: Vec<SnapshotStateRow>,
    fee_pool: u128,
    // Legacy mirror field: accepted on read for compatibility, omitted on write.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    marks_quota: Vec<SnapshotQuotaRow>,
    #[serde(default)]
    imported_set: Vec<[u8; 32]>,
    #[serde(default)]
    exported_registry: Vec<SnapshotExportRow>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct SnapshotAccountWire {
    signing_pubkey: [u8; 32],
    derivation_index: u32,
    balance_pwm: u128,
    staked: u128,
    marks: u32,
    initialized: bool,
    index: u32,
    flags: u32,
    nonce: u64,
    #[serde(default)]
    last_claim_unix_time: u64,
    #[serde(default)]
    last_claim_anchor_ref: u64,
    #[serde(default, rename = "last_free_claim_utc_day")]
    free_claim_utc_day: Option<u64>,
    #[serde(default)]
    last_stake_change_height: u64,
    #[serde(default)]
    rescue_address: Option<[u8; 32]>,
    #[serde(default)]
    active_policies: u16,
    #[serde(default)]
    dormant_policies: u16,
    #[serde(default)]
    finalized: bool,
    #[serde(default)]
    owner_kind: String,
    #[serde(default)]
    owner_display_name: String,
    #[serde(default)]
    owner_country_hint: String,
    #[serde(default)]
    company_metadata_commitment: Option<[u8; 32]>,
    #[serde(default)]
    external_verification_ref: Option<String>,
    #[serde(default)]
    requested_domain_lo: Option<u8>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct SnapshotExportRow {
    export_id: [u8; 32],
    to: [u8; 32],
    target_domain: u16,
    amount: u128,
}

impl From<Account> for SnapshotAccountWire {
    fn from(value: Account) -> Self {
        Self {
            signing_pubkey: value.signing_pubkey,
            derivation_index: value.derivation_index,
            balance_pwm: value.balance_pwm,
            staked: value.staked,
            marks: value.marks,
            initialized: value.initialized,
            index: value.index,
            flags: value.flags,
            nonce: value.nonce,
            last_claim_unix_time: value.last_claim_unix_time,
            last_claim_anchor_ref: value.last_claim_anchor_ref,
            free_claim_utc_day: value.free_claim_utc_day,
            last_stake_change_height: value.last_stake_change_height,
            rescue_address: value.rescue_address,
            active_policies: value.active_policies,
            dormant_policies: value.dormant_policies,
            finalized: value.finalized,
            owner_kind: value.owner_kind,
            owner_display_name: value.owner_display_name,
            owner_country_hint: value.owner_country_hint,
            company_metadata_commitment: value.company_metadata_commitment,
            external_verification_ref: value.external_verification_ref,
            requested_domain_lo: value.requested_domain_lo,
            ..Default::default()
        }
    }
}

impl From<SnapshotAccountWire> for Account {
    fn from(value: SnapshotAccountWire) -> Self {
        Self {
            signing_pubkey: value.signing_pubkey,
            derivation_index: value.derivation_index,
            balance_pwm: value.balance_pwm,
            staked: value.staked,
            marks: value.marks,
            initialized: value.initialized,
            index: value.index,
            flags: value.flags,
            nonce: value.nonce,
            last_claim_unix_time: value.last_claim_unix_time,
            last_claim_anchor_ref: value.last_claim_anchor_ref,
            free_claim_utc_day: value.free_claim_utc_day,
            last_stake_change_height: value.last_stake_change_height,
            rescue_address: value.rescue_address,
            active_policies: value.active_policies,
            dormant_policies: value.dormant_policies,
            finalized: value.finalized,
            owner_kind: value.owner_kind,
            owner_display_name: value.owner_display_name,
            owner_country_hint: value.owner_country_hint,
            company_metadata_commitment: value.company_metadata_commitment,
            external_verification_ref: value.external_verification_ref,
            requested_domain_lo: value.requested_domain_lo,
            ..Account::default()
        }
    }
}

fn serialize_snapshot_state<S>(state: &ChainState, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let accounts = state
        .accounts
        .iter()
        .map(|(id, account)| SnapshotStateRow {
            id: *id,
            account: account.clone().into(),
        })
        .collect();
    SnapshotStateWire {
        accounts,
        fee_pool: state.fee_pool,
        // marks_quota is a removed legacy mirror; canonical snapshots no longer emit it.
        marks_quota: Vec::new(),
        imported_set: state.imported_set.iter().copied().collect(),
        exported_registry: state
            .exported_registry
            .iter()
            .map(|(export_id, row)| SnapshotExportRow {
                export_id: *export_id,
                to: row.to,
                target_domain: row.target_domain,
                amount: row.amount,
            })
            .collect(),
    }
    .serialize(serializer)
}

fn deserialize_snapshot_state<'de, D>(deserializer: D) -> Result<ChainState, D::Error>
where
    D: Deserializer<'de>,
{
    let wire = SnapshotStateWire::deserialize(deserializer)?;
    let mut accounts = BTreeMap::new();
    for row in wire.accounts {
        if accounts.insert(row.id, row.account.into()).is_some() {
            return Err(serde::de::Error::custom(format!(
                "snapshot state contract error: duplicate account id {} in state.accounts",
                hex::encode(row.id)
            )));
        }
    }
    validate_quota_rows(wire.marks_quota, &accounts).map_err(serde::de::Error::custom)?;
    let mut imported_set = BTreeSet::new();
    for export_id in wire.imported_set {
        if !imported_set.insert(export_id) {
            return Err(serde::de::Error::custom(format!(
                "snapshot state contract error: duplicate export id {} in state.imported_set",
                hex::encode(export_id)
            )));
        }
    }
    let mut exported_registry = BTreeMap::new();
    for row in wire.exported_registry {
        if exported_registry
            .insert(
                row.export_id,
                pwm_core::state::ExportProvenance {
                    to: row.to,
                    target_domain: row.target_domain,
                    amount: row.amount,
                },
            )
            .is_some()
        {
            return Err(serde::de::Error::custom(format!(
                "snapshot state contract error: duplicate export id {} in state.exported_registry",
                hex::encode(row.export_id)
            )));
        }
    }
    Ok(ChainState {
        accounts,
        fee_pool: wire.fee_pool,
        imported_set,
        exported_registry,
    })
}

fn hex_of<const N: usize>(bytes: &[u8; N]) -> String {
    hex::encode(bytes)
}

fn dec_of(value: u128) -> String {
    value.to_string()
}

fn dec_of_u32(value: u32) -> String {
    value.to_string()
}

fn hex_v2<const N: usize>(value: &str, field: &str) -> Result<[u8; N], String> {
    let want = N * 2;
    if value.len() != want {
        return Err(format!(
            "{field}: invalid hex length {}, expected {want}",
            value.len()
        ));
    }
    if !value
        .bytes()
        .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
    {
        return Err(format!(
            "{field}: invalid hex; expected lowercase [0-9a-f] without 0x"
        ));
    }
    let decoded = hex::decode(value).map_err(|e| format!("{field}: invalid hex: {e}"))?;
    let mut out = [0u8; N];
    out.copy_from_slice(&decoded);
    Ok(out)
}

fn dec_v2(value: &str, field: &str) -> Result<u128, String> {
    if value.is_empty() {
        return Err(format!("{field}: invalid decimal string: empty"));
    }
    if !value.bytes().all(|b| b.is_ascii_digit()) {
        return Err(format!(
            "{field}: invalid decimal string; expected digits only"
        ));
    }
    if value.len() > 1 && value.as_bytes()[0] == b'0' {
        return Err(format!(
            "{field}: invalid decimal string; leading zeros are not canonical"
        ));
    }
    value
        .parse::<u128>()
        .map_err(|e| format!("{field}: invalid u128 decimal string: {e}"))
}

fn dec_v2_marks(value: &str, field: &str) -> Result<u32, String> {
    let raw = dec_v2(value, field)?;
    if raw <= u32::MAX as u128 {
        return Ok(raw as u32);
    }
    let scaled = raw / pwm_core::PWM_RAW_SCALE;
    Ok(scaled.min(u32::MAX as u128) as u32)
}

fn dec_v2_u32(value: &str, field: &str) -> Result<u32, String> {
    let raw = dec_v2(value, field)?;
    Ok(raw.min(u32::MAX as u128) as u32)
}

fn policy_kind_to_str(kind: PolicyKind) -> &'static str {
    match kind {
        PolicyKind::RoutingSameDomainOnly => "routing.same_domain_only",
        PolicyKind::RoutingEmergencyRedirect => "routing.emergency_redirect",
        PolicyKind::SenderFilter => "sender_filter",
        PolicyKind::DefaultBehavior => "default_behavior",
        PolicyKind::CosignRequired => "cosign_required",
    }
}

fn policy_kind_from_str(raw: &str, path: &str) -> Result<PolicyKind, String> {
    match raw {
        "routing.same_domain_only" => Ok(PolicyKind::RoutingSameDomainOnly),
        "routing.emergency_redirect" => Ok(PolicyKind::RoutingEmergencyRedirect),
        "sender_filter" => Ok(PolicyKind::SenderFilter),
        "default_behavior" => Ok(PolicyKind::DefaultBehavior),
        "cosign_required" => Ok(PolicyKind::CosignRequired),
        _ => Err(format!("{path}: unknown policy kind '{raw}'")),
    }
}

fn activation_to_str(mode: ActivationMode) -> &'static str {
    match mode {
        ActivationMode::Dormant => "dormant",
        ActivationMode::Immediately => "immediately",
    }
}

fn activation_from_str(raw: &str, path: &str) -> Result<ActivationMode, String> {
    match raw {
        "dormant" => Ok(ActivationMode::Dormant),
        "immediately" => Ok(ActivationMode::Immediately),
        _ => Err(format!("{path}: expected dormant|immediately")),
    }
}

fn policy_action_to_v2(value: &PolicyAction) -> PolicyActionV2 {
    match value {
        PolicyAction::SetPolicy { policy, activation } => PolicyActionV2::SetPolicy {
            policy: policy_kind_to_str(*policy).to_string(),
            activation: activation_to_str(*activation).to_string(),
        },
        PolicyAction::ActivatePolicy { policy_id } => PolicyActionV2::ActivatePolicy {
            policy_id: *policy_id,
        },
        PolicyAction::DeactivatePolicy { policy_id } => PolicyActionV2::DeactivatePolicy {
            policy_id: *policy_id,
        },
    }
}

fn policy_action_from_v2(value: PolicyActionV2, path: &str) -> Result<PolicyAction, String> {
    match value {
        PolicyActionV2::SetPolicy { policy, activation } => Ok(PolicyAction::SetPolicy {
            policy: policy_kind_from_str(&policy, &format!("{path}.policy"))?,
            activation: activation_from_str(&activation, &format!("{path}.activation"))?,
        }),
        PolicyActionV2::ActivatePolicy { policy_id } => {
            Ok(PolicyAction::ActivatePolicy { policy_id })
        }
        PolicyActionV2::DeactivatePolicy { policy_id } => {
            Ok(PolicyAction::DeactivatePolicy { policy_id })
        }
    }
}

fn init_v4_to_v2(value: &InitV4Extension) -> InitV4ExtV2 {
    InitV4ExtV2 {
        owner_kind: value.owner_kind.clone(),
        owner_display_name: value.owner_display_name.clone(),
        owner_country_hint: value.owner_country_hint.clone(),
        company_metadata_commitment: hex_of(&value.company_metadata_commitment),
        external_verification_ref: value.external_verification_ref.clone(),
        requested_domain_lo: value.requested_domain_lo,
        rescue_address: value.rescue_address.as_ref().map(hex_of),
        initial_policies: value
            .initial_policies
            .iter()
            .map(|row| InitPolicyV2 {
                policy: policy_kind_to_str(row.policy).to_string(),
                activation: activation_to_str(row.activation).to_string(),
            })
            .collect(),
        cosign_policy: value.cosign_policy.as_ref().map(|x| CosignPolicyV2 {
            min_signers: x.min_signers,
        }),
    }
}

fn init_v4_from_v2(value: InitV4ExtV2, path: &str) -> Result<InitV4Extension, String> {
    Ok(InitV4Extension {
        owner_kind: value.owner_kind,
        owner_display_name: value.owner_display_name,
        owner_country_hint: value.owner_country_hint,
        company_metadata_commitment: hex_v2(
            &value.company_metadata_commitment,
            &format!("{path}.company_metadata_commitment"),
        )?,
        external_verification_ref: value.external_verification_ref,
        requested_domain_lo: value.requested_domain_lo,
        rescue_address: value
            .rescue_address
            .as_deref()
            .map(|v| hex_v2(v, &format!("{path}.rescue_address")))
            .transpose()?,
        initial_policies: value
            .initial_policies
            .into_iter()
            .enumerate()
            .map(|(i, row)| {
                Ok(InitPolicyEntry {
                    policy: policy_kind_from_str(
                        &row.policy,
                        &format!("{path}.initial_policies[{i}].policy"),
                    )?,
                    activation: activation_from_str(
                        &row.activation,
                        &format!("{path}.initial_policies[{i}].activation"),
                    )?,
                })
            })
            .collect::<Result<Vec<_>, String>>()?,
        cosign_policy: value.cosign_policy.map(|x| CosignPolicy {
            min_signers: x.min_signers,
        }),
    })
}

fn validate_quota_rows(
    rows: Vec<SnapshotQuotaRow>,
    accounts: &BTreeMap<[u8; 32], Account>,
) -> Result<(), String> {
    let mut seen = BTreeSet::new();
    for row in rows {
        if !seen.insert(row.id) {
            return Err(format!(
                "snapshot state contract error: duplicate account id {} in state.marks_quota",
                hex::encode(row.id)
            ));
        }
        let Some(account) = accounts.get(&row.id) else {
            return Err(format!(
                "snapshot state contract error: marks_quota id {} is not present in state.accounts",
                hex::encode(row.id)
            ));
        };
        if row.quota != account.marks {
            return Err(format!(
                "snapshot state contract error: marks_quota mismatch for id {}: quota={} marks={}",
                hex::encode(row.id),
                row.quota,
                account.marks
            ));
        }
    }
    Ok(())
}

pub(super) fn data_to_v2(value: &SnapshotData) -> SnapshotDataV2 {
    SnapshotDataV2 {
        version: SNAPSHOT_VERSION,
        genesis_accounts: value
            .genesis_accounts
            .iter()
            .map(|row| SnapshotGenesisRowV2 {
                acct: hex_of(&row.acct),
                pubkey: hex_of(&row.pubkey),
                der_idx: row.der_idx,
            })
            .collect(),
        blocks: value.blocks.iter().map(block_to_v2).collect(),
        state: state_to_v2(&value.state),
        roaming: roaming_to_v2(&value.roaming),
        cross_shard: cross_shard_to_v2(&value.cross_shard),
        blocks_stored: value.blocks_stored,
        checkpoint_height: value.checkpoint_height,
    }
}

pub(super) fn data_from_v2(value: SnapshotDataV2) -> Result<SnapshotData, String> {
    if value.version != SNAPSHOT_VERSION {
        return Err(format!(
            "version: unsupported snapshot version {}, expected {}",
            value.version, SNAPSHOT_VERSION
        ));
    }
    Ok(SnapshotData {
        version: SNAPSHOT_VERSION,
        genesis_accounts: value
            .genesis_accounts
            .into_iter()
            .enumerate()
            .map(|(i, row)| {
                Ok(SnapshotGenesisRow {
                    acct: hex_v2(&row.acct, &format!("genesis_accounts[{i}].acct"))?,
                    pubkey: hex_v2(&row.pubkey, &format!("genesis_accounts[{i}].pubkey"))?,
                    der_idx: row.der_idx,
                })
            })
            .collect::<Result<Vec<_>, String>>()?,
        blocks: value
            .blocks
            .into_iter()
            .enumerate()
            .map(|(i, block)| block_from_v2(block, &format!("blocks[{i}]")))
            .collect::<Result<Vec<_>, String>>()?,
        state: state_from_v2(value.state, "state")?,
        roaming: roaming_from_v2(value.roaming)?,
        cross_shard: cross_shard_from_v2(value.cross_shard)?,
        blocks_stored: value.blocks_stored,
        checkpoint_height: value.checkpoint_height,
    })
}

fn block_to_v2(value: &Block) -> BlockV2 {
    BlockV2 {
        hdr: hdr_to_v2(&value.hdr),
        txs: value.txs.iter().map(tx_to_v2).collect(),
    }
}

fn block_from_v2(value: BlockV2, path: &str) -> Result<Block, String> {
    Ok(Block {
        hdr: hdr_from_v2(value.hdr, &format!("{path}.hdr"))?,
        txs: value
            .txs
            .into_iter()
            .enumerate()
            .map(|(i, tx)| tx_from_v2(tx, &format!("{path}.txs[{i}]")))
            .collect::<Result<Vec<_>, String>>()?,
    })
}

fn hdr_to_v2(value: &BlockHdr) -> BlockHdrV2 {
    BlockHdrV2 {
        height: value.height,
        prev_hash: hex_of(&value.prev_hash),
        ts: value.ts,
        prod_idx: value.prod_idx,
        tx_root: hex_of(&value.tx_root),
        state_root: hex_of(&value.state_root),
        sig: hex_of(&value.sig),
    }
}

fn hdr_from_v2(value: BlockHdrV2, path: &str) -> Result<BlockHdr, String> {
    Ok(BlockHdr {
        height: value.height,
        prev_hash: hex_v2(&value.prev_hash, &format!("{path}.prev_hash"))?,
        ts: value.ts,
        prod_idx: value.prod_idx,
        tx_root: hex_v2(&value.tx_root, &format!("{path}.tx_root"))?,
        state_root: hex_v2(&value.state_root, &format!("{path}.state_root"))?,
        sig: hex_v2(&value.sig, &format!("{path}.sig"))?,
    })
}

fn tx_to_v2(value: &SignedTx) -> SignedTxV2 {
    SignedTxV2 {
        domain_code: value.domain_code,
        signer_pk: hex_of(&value.signer_pk),
        derivation_index: value.derivation_index,
        nonce: value.nonce,
        body: body_to_v2(&value.body),
        burn_purpose: value.burn_purpose.clone(),
        import_fee: value.import_fee.map(dec_of),
        import_provenance: value.import_provenance.as_ref().map(|prov| ImportProvV2 {
            to: hex_of(&prov.to),
            target_domain: prov.target_domain,
            amount: dec_of(prov.amount),
        }),
        init_v4: value.init_v4.as_ref().map(init_v4_to_v2),
        cosigns: value.cosigns.iter().map(cosign_to_v2).collect(),
        signature: hex_of(&value.signature),
    }
}

fn tx_from_v2(value: SignedTxV2, path: &str) -> Result<SignedTx, String> {
    Ok(SignedTx {
        domain_code: value.domain_code,
        signer_pk: hex_v2(&value.signer_pk, &format!("{path}.signer_pk"))?,
        derivation_index: value.derivation_index,
        nonce: value.nonce,
        body: body_from_v2(value.body, &format!("{path}.body"))?,
        burn_purpose: value.burn_purpose,
        import_fee: value
            .import_fee
            .as_deref()
            .map(|v| dec_v2(v, &format!("{path}.import_fee")))
            .transpose()?,
        import_provenance: match value.import_provenance {
            Some(prov) => Some(pwm_core::state::ExportProvenance {
                to: hex_v2(&prov.to, &format!("{path}.import_provenance.to"))?,
                target_domain: prov.target_domain,
                amount: dec_v2(&prov.amount, &format!("{path}.import_provenance.amount"))?,
            }),
            None => None,
        },
        init_v4: value
            .init_v4
            .map(|v| init_v4_from_v2(v, &format!("{path}.init_v4")))
            .transpose()?,
        cosigns: value
            .cosigns
            .into_iter()
            .enumerate()
            .map(|(i, row)| cosign_from_v2(row, &format!("{path}.cosigns[{i}]")))
            .collect::<Result<Vec<_>, String>>()?,
        signature: hex_v2(&value.signature, &format!("{path}.signature"))?,
    })
}

fn cosign_to_v2(value: &Cosignature) -> CosignatureV2 {
    CosignatureV2 {
        signer_pk: hex_of(&value.signer_pk),
        role: value.role,
        signature: hex_of(&value.signature),
    }
}

fn cosign_from_v2(value: CosignatureV2, path: &str) -> Result<Cosignature, String> {
    Ok(Cosignature {
        signer_pk: hex_v2(&value.signer_pk, &format!("{path}.signer_pk"))?,
        role: value.role,
        signature: hex_v2(&value.signature, &format!("{path}.signature"))?,
    })
}

fn body_to_v2(value: &TxBody) -> TxBodyV2 {
    match value {
        TxBody::Init { index, flags } => TxBodyV2::Init {
            index: *index,
            flags: *flags,
        },
        TxBody::Transfer { to, amount, fee } => TxBodyV2::Transfer {
            to: hex_of(to),
            amount: dec_of(*amount),
            fee: dec_of(*fee),
        },
        TxBody::Stake { amount } => TxBodyV2::Stake {
            amount: dec_of(*amount),
        },
        TxBody::Unstake { amount } => TxBodyV2::Unstake {
            amount: dec_of(*amount),
        },
        TxBody::BurnMark {
            mark_amount,
            beneficiary,
        } => TxBodyV2::BurnMark {
            mark_amount: dec_of_u32(*mark_amount),
            beneficiary: beneficiary.as_ref().map(hex_of),
        },
        TxBody::Claim {
            mode,
            claim_units,
            anchor_ref,
            fee,
        } => TxBodyV2::Claim {
            mode: match mode {
                pwm_core::tx::ClaimMode::Free => "free".to_string(),
                pwm_core::tx::ClaimMode::Paid => "paid".to_string(),
            },
            claim_units: dec_of_u32(*claim_units),
            anchor_ref: *anchor_ref,
            fee: dec_of(*fee),
        },
        TxBody::Export {
            to,
            target_domain,
            amount,
            fee,
        } => TxBodyV2::Export {
            to: hex_of(to),
            target_domain: *target_domain,
            amount: dec_of(*amount),
            fee: dec_of(*fee),
        },
        TxBody::Import {
            to,
            amount,
            export_id,
        } => TxBodyV2::Import {
            to: hex_of(to),
            amount: dec_of(*amount),
            export_id: hex_of(export_id),
        },
        TxBody::Policy {
            target_account,
            action,
            fee,
        } => TxBodyV2::Policy {
            target_account: hex_of(target_account),
            action: policy_action_to_v2(action),
            fee: dec_of(*fee),
        },
    }
}

fn body_from_v2(value: TxBodyV2, path: &str) -> Result<TxBody, String> {
    match value {
        TxBodyV2::Init { index, flags } => Ok(TxBody::Init { index, flags }),
        TxBodyV2::Transfer { to, amount, fee } => Ok(TxBody::Transfer {
            to: hex_v2(&to, &format!("{path}.to"))?,
            amount: dec_v2(&amount, &format!("{path}.amount"))?,
            fee: dec_v2(&fee, &format!("{path}.fee"))?,
        }),
        TxBodyV2::Stake { amount } => Ok(TxBody::Stake {
            amount: dec_v2(&amount, &format!("{path}.amount"))?,
        }),
        TxBodyV2::Unstake { amount } => Ok(TxBody::Unstake {
            amount: dec_v2(&amount, &format!("{path}.amount"))?,
        }),
        TxBodyV2::BurnMark {
            mark_amount,
            beneficiary,
        } => Ok(TxBody::BurnMark {
            mark_amount: dec_v2_u32(&mark_amount, &format!("{path}.mark_amount"))?,
            beneficiary: beneficiary
                .as_deref()
                .map(|v| hex_v2(v, &format!("{path}.beneficiary")))
                .transpose()?,
        }),
        TxBodyV2::Claim {
            mode,
            claim_units,
            anchor_ref,
            fee,
        } => Ok(TxBody::Claim {
            mode: match mode.as_str() {
                "free" => pwm_core::tx::ClaimMode::Free,
                "paid" => pwm_core::tx::ClaimMode::Paid,
                _ => return Err(format!("{path}.mode: expected free|paid")),
            },
            claim_units: dec_v2_u32(&claim_units, &format!("{path}.claim_units"))?,
            anchor_ref,
            fee: dec_v2(&fee, &format!("{path}.fee"))?,
        }),
        TxBodyV2::Export {
            to,
            target_domain,
            amount,
            fee,
        } => Ok(TxBody::Export {
            to: hex_v2(&to, &format!("{path}.to"))?,
            target_domain,
            amount: dec_v2(&amount, &format!("{path}.amount"))?,
            fee: dec_v2(&fee, &format!("{path}.fee"))?,
        }),
        TxBodyV2::Import {
            to,
            amount,
            export_id,
        } => Ok(TxBody::Import {
            to: hex_v2(&to, &format!("{path}.to"))?,
            amount: dec_v2(&amount, &format!("{path}.amount"))?,
            export_id: hex_v2(&export_id, &format!("{path}.export_id"))?,
        }),
        TxBodyV2::Policy {
            target_account,
            action,
            fee,
        } => Ok(TxBody::Policy {
            target_account: hex_v2(&target_account, &format!("{path}.target_account"))?,
            action: policy_action_from_v2(action, &format!("{path}.action"))?,
            fee: dec_v2(&fee, &format!("{path}.fee"))?,
        }),
    }
}

fn state_to_v2(value: &ChainState) -> SnapshotStateV2 {
    SnapshotStateV2 {
        accounts: value
            .accounts
            .iter()
            .map(|(id, account)| SnapshotStateRowV2 {
                id: hex_of(id),
                account: account_to_v2(account),
            })
            .collect(),
        fee_pool: dec_of(value.fee_pool),
        // marks_quota is a removed legacy mirror; canonical snapshots no longer emit it.
        marks_quota: Vec::new(),
        imported_set: value.imported_set.iter().map(hex_of).collect(),
        exported_registry: value
            .exported_registry
            .iter()
            .map(|(export_id, row)| SnapshotExportRowV2 {
                export_id: hex_of(export_id),
                to: hex_of(&row.to),
                target_domain: row.target_domain,
                amount: dec_of(row.amount),
            })
            .collect(),
    }
}

fn state_from_v2(value: SnapshotStateV2, path: &str) -> Result<ChainState, String> {
    let wire = SnapshotStateWire {
        accounts: value
            .accounts
            .into_iter()
            .enumerate()
            .map(|(i, row)| {
                Ok(SnapshotStateRow {
                    id: hex_v2(&row.id, &format!("{path}.accounts[{i}].id"))?,
                    account: account_from_v2(
                        row.account,
                        &format!("{path}.accounts[{i}].account"),
                    )?,
                })
            })
            .collect::<Result<Vec<_>, String>>()?,
        fee_pool: dec_v2(&value.fee_pool, &format!("{path}.fee_pool"))?,
        marks_quota: value
            .marks_quota
            .into_iter()
            .enumerate()
            .map(|(i, row)| {
                Ok(SnapshotQuotaRow {
                    id: hex_v2(&row.id, &format!("{path}.marks_quota[{i}].id"))?,
                    quota: dec_v2_marks(&row.quota, &format!("{path}.marks_quota[{i}].quota"))?,
                })
            })
            .collect::<Result<Vec<_>, String>>()?,
        imported_set: value
            .imported_set
            .into_iter()
            .enumerate()
            .map(|(i, id)| hex_v2(&id, &format!("{path}.imported_set[{i}]")))
            .collect::<Result<Vec<_>, String>>()?,
        exported_registry: value
            .exported_registry
            .into_iter()
            .enumerate()
            .map(|(i, row)| {
                Ok(SnapshotExportRow {
                    export_id: hex_v2(
                        &row.export_id,
                        &format!("{path}.exported_registry[{i}].export_id"),
                    )?,
                    to: hex_v2(&row.to, &format!("{path}.exported_registry[{i}].to"))?,
                    target_domain: row.target_domain,
                    amount: dec_v2(
                        &row.amount,
                        &format!("{path}.exported_registry[{i}].amount"),
                    )?,
                })
            })
            .collect::<Result<Vec<_>, String>>()?,
    };
    state_from_wire(wire).map_err(|e| format!("{path}: {e}"))
}

fn account_to_v2(value: &Account) -> SnapshotAccountV2 {
    SnapshotAccountV2 {
        signing_pubkey: hex_of(&value.signing_pubkey),
        derivation_index: value.derivation_index,
        balance_pwm: dec_of(value.balance_pwm),
        staked: dec_of(value.staked),
        marks: dec_of_u32(value.marks),
        initialized: value.initialized,
        index: value.index,
        flags: value.flags,
        nonce: value.nonce,
        last_claim_unix_time: value.last_claim_unix_time,
        last_claim_anchor_ref: value.last_claim_anchor_ref,
        free_claim_utc_day: value.free_claim_utc_day,
        last_stake_change_height: value.last_stake_change_height,
        rescue_address: value.rescue_address.as_ref().map(hex_of),
        active_policies: value.active_policies,
        dormant_policies: value.dormant_policies,
        finalized: value.finalized,
        owner_kind: value.owner_kind.clone(),
        owner_display_name: value.owner_display_name.clone(),
        owner_country_hint: value.owner_country_hint.clone(),
        company_metadata_commitment: value.company_metadata_commitment.as_ref().map(hex_of),
        external_verification_ref: value.external_verification_ref.clone(),
        requested_domain_lo: value.requested_domain_lo,
        ..Default::default()
    }
}

fn account_from_v2(value: SnapshotAccountV2, path: &str) -> Result<SnapshotAccountWire, String> {
    Ok(SnapshotAccountWire {
        signing_pubkey: hex_v2(&value.signing_pubkey, &format!("{path}.signing_pubkey"))?,
        derivation_index: value.derivation_index,
        balance_pwm: dec_v2(&value.balance_pwm, &format!("{path}.balance_pwm"))?,
        staked: dec_v2(&value.staked, &format!("{path}.staked"))?,
        marks: dec_v2_marks(&value.marks, &format!("{path}.marks"))?,
        initialized: value.initialized,
        index: value.index,
        flags: value.flags,
        nonce: value.nonce,
        last_claim_unix_time: value.last_claim_unix_time,
        last_claim_anchor_ref: value.last_claim_anchor_ref,
        free_claim_utc_day: value.free_claim_utc_day,
        last_stake_change_height: value.last_stake_change_height,
        rescue_address: value
            .rescue_address
            .as_deref()
            .map(|v| hex_v2(v, &format!("{path}.rescue_address")))
            .transpose()?,
        active_policies: value.active_policies,
        dormant_policies: value.dormant_policies,
        finalized: value.finalized,
        owner_kind: value.owner_kind,
        owner_display_name: value.owner_display_name,
        owner_country_hint: value.owner_country_hint,
        company_metadata_commitment: value
            .company_metadata_commitment
            .as_deref()
            .map(|v| hex_v2(v, &format!("{path}.company_metadata_commitment")))
            .transpose()?,
        external_verification_ref: value.external_verification_ref,
        requested_domain_lo: value.requested_domain_lo,
    })
}

fn roaming_to_v2(value: &SnapshotRoamingWire) -> SnapshotRoamingV2 {
    SnapshotRoamingV2 {
        intents: value
            .intents
            .iter()
            .map(|row| SnapshotIntentRowV2 {
                intent_id: hex_of(&row.intent_id),
                export_id: hex_of(&row.export_id),
                source: hex_of(&row.source),
                to: hex_of(&row.to),
                target_domain: row.target_domain,
                amount: dec_of(row.amount),
                fee: dec_of(row.fee),
                status: row.status,
                created_height: row.created_height,
                expires_at_height: row.expires_at_height,
                last_error: row.last_error.clone(),
            })
            .collect(),
        locks: value
            .locks
            .iter()
            .map(|row| SnapshotIntentLockRowV2 {
                source: hex_of(&row.source),
                intent_id: hex_of(&row.intent_id),
            })
            .collect(),
    }
}

fn roaming_from_v2(value: SnapshotRoamingV2) -> Result<SnapshotRoamingWire, String> {
    Ok(SnapshotRoamingWire {
        intents: value
            .intents
            .into_iter()
            .enumerate()
            .map(|(i, row)| {
                Ok(SnapshotIntentRow {
                    intent_id: hex_v2(&row.intent_id, &format!("roaming.intents[{i}].intent_id"))?,
                    export_id: hex_v2(&row.export_id, &format!("roaming.intents[{i}].export_id"))?,
                    source: hex_v2(&row.source, &format!("roaming.intents[{i}].source"))?,
                    to: hex_v2(&row.to, &format!("roaming.intents[{i}].to"))?,
                    target_domain: row.target_domain,
                    amount: dec_v2(&row.amount, &format!("roaming.intents[{i}].amount"))?,
                    fee: dec_v2(&row.fee, &format!("roaming.intents[{i}].fee"))?,
                    status: row.status,
                    created_height: row.created_height,
                    expires_at_height: row.expires_at_height,
                    last_error: row.last_error,
                })
            })
            .collect::<Result<Vec<_>, String>>()?,
        locks: value
            .locks
            .into_iter()
            .enumerate()
            .map(|(i, row)| {
                Ok(SnapshotIntentLockRow {
                    source: hex_v2(&row.source, &format!("roaming.locks[{i}].source"))?,
                    intent_id: hex_v2(&row.intent_id, &format!("roaming.locks[{i}].intent_id"))?,
                })
            })
            .collect::<Result<Vec<_>, String>>()?,
    })
}

fn state_from_wire(wire: SnapshotStateWire) -> Result<ChainState, String> {
    let mut accounts = BTreeMap::new();
    for row in wire.accounts {
        if accounts.insert(row.id, row.account.into()).is_some() {
            return Err(format!(
                "snapshot state contract error: duplicate account id {} in state.accounts",
                hex::encode(row.id)
            ));
        }
    }
    validate_quota_rows(wire.marks_quota, &accounts)?;
    let mut imported_set = BTreeSet::new();
    for export_id in wire.imported_set {
        if !imported_set.insert(export_id) {
            return Err(format!(
                "snapshot state contract error: duplicate export id {} in state.imported_set",
                hex::encode(export_id)
            ));
        }
    }
    let mut exported_registry = BTreeMap::new();
    for row in wire.exported_registry {
        if exported_registry
            .insert(
                row.export_id,
                pwm_core::state::ExportProvenance {
                    to: row.to,
                    target_domain: row.target_domain,
                    amount: row.amount,
                },
            )
            .is_some()
        {
            return Err(format!(
                "snapshot state contract error: duplicate export id {} in state.exported_registry",
                hex::encode(row.export_id)
            ));
        }
    }
    Ok(ChainState {
        accounts,
        fee_pool: wire.fee_pool,
        imported_set,
        exported_registry,
    })
}
impl SnapshotData {
    pub(crate) fn into_runtime(
        self,
    ) -> Result<(Vec<Block>, ChainState, RoamingPool, CrossShardLedger), String> {
        let roaming_pool = roaming_from_wire(self.roaming)?;
        Ok((self.blocks, self.state, roaming_pool, self.cross_shard))
    }
}

#[cfg(test)]
mod tests {
    use super::{
        account_from_v2, account_to_v2, tx_from_v2, tx_to_v2, Account, CosignatureV2, SignedTxV2,
        SnapshotAccountWire,
    };
    use pwm_core::tx::{CosignRole, Cosignature, SignedTx, TxBody};

    #[test]
    fn acct_free_claim_day_rt() {
        let expected_day = 20_510;
        let account = Account {
            free_claim_utc_day: Some(expected_day),
            ..Account::default()
        };

        let wire: SnapshotAccountWire = account.clone().into();
        assert_eq!(wire.free_claim_utc_day, Some(expected_day));

        let from_wire: Account = wire.into();
        assert_eq!(from_wire.free_claim_utc_day, Some(expected_day));

        let v2 = account_to_v2(&from_wire);
        assert_eq!(v2.free_claim_utc_day, Some(expected_day));

        let from_v2 = account_from_v2(v2, "state.accounts[0].account").expect("v2 account parse");
        assert_eq!(from_v2.free_claim_utc_day, Some(expected_day));
    }

    #[test]
    fn tx_cosigns_rt_v2_wire() {
        let mut tx = SignedTx {
            domain_code: 1,
            signer_pk: [1u8; 32],
            derivation_index: 2,
            nonce: 3,
            body: TxBody::Stake { amount: 10 },
            burn_purpose: None,
            import_fee: None,
            import_provenance: None,
            init_v4: None,
            cosigns: vec![Cosignature {
                signer_pk: [2u8; 32],
                role: CosignRole::Organization,
                signature: [3u8; 64],
            }],
            signature: [4u8; 64],
        };

        let wire = tx_to_v2(&tx);
        assert_eq!(wire.cosigns.len(), 1);
        let decoded = tx_from_v2(wire, "blocks[0].txs[0]").expect("tx from v2");
        assert_eq!(decoded.cosigns, tx.cosigns);

        tx.cosigns.clear();
        let legacy = SignedTxV2 {
            domain_code: tx.domain_code,
            signer_pk: hex::encode(tx.signer_pk),
            derivation_index: tx.derivation_index,
            nonce: tx.nonce,
            body: super::body_to_v2(&tx.body),
            burn_purpose: tx.burn_purpose,
            import_fee: None,
            import_provenance: None,
            init_v4: None,
            cosigns: Vec::<CosignatureV2>::new(),
            signature: hex::encode(tx.signature),
        };
        let legacy_decoded = tx_from_v2(legacy, "blocks[0].txs[0]").expect("legacy tx from v2");
        assert!(legacy_decoded.cosigns.is_empty());
    }
}
