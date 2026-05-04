//! serde snapshot envelope: blocks, accounts state, roaming wire, cross-shard.

use crate::ledger::{CrossShardFact, CrossShardLedger, CrossShardOrigin, CrossShardStatus};
use crate::roaming::{IntentStatus, RoamingIntent, RoamingPool};
use pwm_core::block::{Block, BlockHdr};
use pwm_core::tx::{SignedTx, TxBody};
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
    import_provenance: Option<ImportProvV2>,
    signature: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ImportProvV2 {
    to: String,
    target_domain: u16,
    amount: String,
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
    #[serde(default)]
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

#[derive(Clone, Debug, Serialize, Deserialize)]
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
    quota: u128,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct SnapshotStateWire {
    accounts: Vec<SnapshotStateRow>,
    fee_pool: u128,
    #[serde(default)]
    marks_quota: Vec<SnapshotQuotaRow>,
    #[serde(default)]
    imported_set: Vec<[u8; 32]>,
    #[serde(default)]
    exported_registry: Vec<SnapshotExportRow>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct SnapshotAccountWire {
    signing_pubkey: [u8; 32],
    derivation_index: u32,
    balance_pwm: u128,
    staked: u128,
    marks: u128,
    initialized: bool,
    index: u32,
    flags: u32,
    nonce: u64,
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
        marks_quota: state
            .marks_quota
            .iter()
            .map(|(id, quota)| SnapshotQuotaRow {
                id: *id,
                quota: *quota,
            })
            .collect(),
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
    let mut marks_quota = BTreeMap::new();
    for row in wire.marks_quota {
        if marks_quota.insert(row.id, row.quota).is_some() {
            return Err(serde::de::Error::custom(format!(
                "snapshot state contract error: duplicate account id {} in state.marks_quota",
                hex::encode(row.id)
            )));
        }
        if !accounts.contains_key(&row.id) {
            return Err(serde::de::Error::custom(format!(
                "snapshot state contract error: marks_quota id {} is not present in state.accounts",
                hex::encode(row.id)
            )));
        }
    }
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
        marks_quota,
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
        import_provenance: value.import_provenance.as_ref().map(|prov| ImportProvV2 {
            to: hex_of(&prov.to),
            target_domain: prov.target_domain,
            amount: dec_of(prov.amount),
        }),
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
        import_provenance: match value.import_provenance {
            Some(prov) => Some(pwm_core::state::ExportProvenance {
                to: hex_v2(&prov.to, &format!("{path}.import_provenance.to"))?,
                target_domain: prov.target_domain,
                amount: dec_v2(&prov.amount, &format!("{path}.import_provenance.amount"))?,
            }),
            None => None,
        },
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
            mark_amount: dec_of(*mark_amount),
            beneficiary: beneficiary.as_ref().map(hex_of),
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
            mark_amount: dec_v2(&mark_amount, &format!("{path}.mark_amount"))?,
            beneficiary: beneficiary
                .as_deref()
                .map(|v| hex_v2(v, &format!("{path}.beneficiary")))
                .transpose()?,
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
        marks_quota: value
            .marks_quota
            .iter()
            .map(|(id, quota)| SnapshotQuotaRowV2 {
                id: hex_of(id),
                quota: dec_of(*quota),
            })
            .collect(),
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
                    quota: dec_v2(&row.quota, &format!("{path}.marks_quota[{i}].quota"))?,
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
        marks: dec_of(value.marks),
        initialized: value.initialized,
        index: value.index,
        flags: value.flags,
        nonce: value.nonce,
    }
}

fn account_from_v2(value: SnapshotAccountV2, path: &str) -> Result<SnapshotAccountWire, String> {
    Ok(SnapshotAccountWire {
        signing_pubkey: hex_v2(&value.signing_pubkey, &format!("{path}.signing_pubkey"))?,
        derivation_index: value.derivation_index,
        balance_pwm: dec_v2(&value.balance_pwm, &format!("{path}.balance_pwm"))?,
        staked: dec_v2(&value.staked, &format!("{path}.staked"))?,
        marks: dec_v2(&value.marks, &format!("{path}.marks"))?,
        initialized: value.initialized,
        index: value.index,
        flags: value.flags,
        nonce: value.nonce,
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
    let mut marks_quota = BTreeMap::new();
    for row in wire.marks_quota {
        if marks_quota.insert(row.id, row.quota).is_some() {
            return Err(format!(
                "snapshot state contract error: duplicate account id {} in state.marks_quota",
                hex::encode(row.id)
            ));
        }
        if !accounts.contains_key(&row.id) {
            return Err(format!(
                "snapshot state contract error: marks_quota id {} is not present in state.accounts",
                hex::encode(row.id)
            ));
        }
    }
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
        marks_quota,
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
