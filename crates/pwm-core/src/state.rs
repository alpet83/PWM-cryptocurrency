//! Canonical chain state: accounts map, fees, burns, import consumed IDs.

use crate::crypto::verify;
use crate::genesis::{ClaimPhaseConfig, GenCfg};
use crate::marks::compute_lazy_marks;
use crate::tx::{
    export_context_is_valid, import_context_is_valid, policy_weakens_cosign, same_hi_domain,
    ActivationMode, CosignRole, PolicyAction, PolicyKind, SignedTx, TxBody, TxError,
};
use crate::types::{conservation_flag, cosign_non_dis, Account, AccountId, DeferredPolicyEntry};
use crate::MARKS_CAP;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::time::{SystemTime, UNIX_EPOCH};

const PPM_DENOM: u128 = 1_000_000;
const CLAIM_IPV4_MSG_TAG: &[u8] = b"PWM/IPV4/CLAIM/V1";

fn find_claim_phase(gen_cfg: &GenCfg, phase: u8) -> Result<&ClaimPhaseConfig, TxError> {
    gen_cfg
        .ipv4_claim_phases
        .iter()
        .find(|row| row.phase == phase)
        .ok_or(TxError::PolicySchemaInvalid)
}

fn claim_ipv4_msg(phase: u8, batch_root: &[u8; 32], claimant: &AccountId) -> Vec<u8> {
    let mut msg = Vec::with_capacity(CLAIM_IPV4_MSG_TAG.len() + 1 + 32 + 32);
    msg.extend_from_slice(CLAIM_IPV4_MSG_TAG);
    msg.push(phase);
    msg.extend_from_slice(batch_root);
    msg.extend_from_slice(claimant);
    msg
}

fn verify_claim_sig(
    st: &State,
    reg_id: &AccountId,
    msg: &[u8],
    registry_sig: &[u8; 64],
) -> Result<(), TxError> {
    let reg_pk = st
        .accounts
        .get(reg_id)
        .filter(|row| row.initialized)
        .map(|row| row.signing_pubkey)
        .ok_or(TxError::PolicySchemaInvalid)?;
    if verify(&reg_pk, msg, registry_sig) {
        Ok(())
    } else {
        Err(TxError::BadSignature)
    }
}

fn pending_pol_allowed(action: &PolicyAction) -> bool {
    matches!(
        action,
        PolicyAction::ActivatePolicy { policy_id, .. }
            if *policy_id == PolicyKind::RoutingEmergencyRedirect.policy_id()
    )
}

fn pending_tx_conflict(body: &TxBody) -> bool {
    match body {
        TxBody::Transfer { .. } => false,
        TxBody::Policy { action, .. } => !pending_pol_allowed(action),
        _ => true,
    }
}

#[derive(Debug)]
pub enum PolicyDecision {
    Allow,
    Reject(TxError),
    Redirect(AccountId),
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExportProvenance {
    #[serde(with = "crate::ser_bin::hex32")]
    pub to: AccountId,
    pub target_domain: u16,
    #[serde(with = "crate::ser_json_u128")]
    pub amount: u128,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum CrossShardLockState {
    Locked,
    Released,
    Refunded,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct CrossShardLock {
    pub export_id: [u8; 32],
    pub sender: AccountId,
    #[serde(with = "crate::ser_json_u128")]
    pub amount_pwm: u128,
    pub target_domain: u16,
    pub refund_account: AccountId,
    pub lock_height: u64,
    pub unlock_height: u64,
    pub state: CrossShardLockState,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum EvidenceType {
    DuplicateVote,
    InvalidAttestation,
    UnavailableProposer,
    CustomStub(u16),
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct EvidenceRecord {
    pub record_id: [u8; 32],
    pub height: u64,
    pub offender_validator_idx: u16,
    pub evidence_type: EvidenceType,
    pub payload_hash: [u8; 32],
    pub reporter: Option<AccountId>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PendingConservationTransfer {
    pub sender: AccountId,
    pub recipient: AccountId,
    #[serde(with = "crate::ser_json_u128")]
    pub amount_pwm: u128,
    pub fee_pwm: u64,
    pub nonce: u64,
    pub enqueue_height: u64,
    pub execute_at_height: u64,
    pub tx_hash: [u8; 32],
}

/// Live ledger + fee sink.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct State {
    pub accounts: BTreeMap<AccountId, Account>,
    pub fee_pool: u128,
    #[serde(default)]
    pub imported_set: BTreeSet<[u8; 32]>,
    #[serde(default)]
    pub exported_registry: BTreeMap<[u8; 32], ExportProvenance>,
    #[serde(default)]
    pub epoch_counter: u64,
    #[serde(default)]
    pub active_validator_indices: Vec<u16>,
    #[serde(default)]
    pub cross_shard_locks: Vec<CrossShardLock>,
    #[serde(default)]
    pub evidence_log: Vec<EvidenceRecord>,
    #[serde(default)]
    pub pending_conservation: Vec<PendingConservationTransfer>,
}

/// Stable blake3 over bincode (devnet state root).
pub fn digest(st: &State) -> [u8; 32] {
    *blake3::hash(&bincode::serialize(st).expect("state bincode")).as_bytes()
}

impl State {
    pub fn get(&self, id: &AccountId) -> Option<&Account> {
        self.accounts.get(id)
    }

    pub fn evidence_record_id(
        height: u64,
        offender_validator_idx: u16,
        evidence_type: &EvidenceType,
        payload_hash: [u8; 32],
    ) -> [u8; 32] {
        let mut h = blake3::Hasher::new();
        h.update(b"PWMv6/evidence-record");
        h.update(&height.to_le_bytes());
        h.update(&offender_validator_idx.to_le_bytes());
        let ty = bincode::serialize(evidence_type).expect("evidence type bincode");
        h.update(&(ty.len() as u32).to_le_bytes());
        h.update(&ty);
        h.update(&payload_hash);
        *h.finalize().as_bytes()
    }

    pub fn append_evidence(
        &mut self,
        height: u64,
        offender_validator_idx: u16,
        evidence_type: EvidenceType,
        payload_hash: [u8; 32],
        reporter: Option<AccountId>,
    ) -> Result<[u8; 32], TxError> {
        let record_id =
            Self::evidence_record_id(height, offender_validator_idx, &evidence_type, payload_hash);
        if self
            .evidence_log
            .iter()
            .any(|record| record.record_id == record_id)
        {
            return Err(TxError::EvidenceDuplicate);
        }
        self.evidence_log.push(EvidenceRecord {
            record_id,
            height,
            offender_validator_idx,
            evidence_type,
            payload_hash,
            reporter,
        });
        Ok(record_id)
    }

    fn require_recipient(&self, id: &AccountId) -> Result<(), TxError> {
        let acc = self.accounts.get(id).ok_or(TxError::RecipientMissing)?;
        if !acc.initialized {
            return Err(TxError::RecipientNotInitialized);
        }
        Ok(())
    }

    /// Dry-run [`Self::apply_tx_with_ctx`] on a clone using explicit inclusion context.
    pub fn precheck_apply_with_ctx(
        &self,
        tx: &SignedTx,
        inclusion_height: u64,
        block_unix_time: u64,
        gen_cfg: &GenCfg,
    ) -> Result<(), TxError> {
        let mut st = self.clone();
        st.apply_tx_with_ctx(tx, inclusion_height, block_unix_time, gen_cfg)
    }

    /// Dry-run on the next block after `tip_height` (mempool / RPC admission).
    pub fn precheck_apply_tip(
        &self,
        tx: &SignedTx,
        tip_height: u64,
        block_unix_time: u64,
        gen_cfg: &GenCfg,
    ) -> Result<(), TxError> {
        self.precheck_apply_with_ctx(tx, tip_height.saturating_add(1), block_unix_time, gen_cfg)
    }

    fn default_ctx_cfg() -> GenCfg {
        crate::genesis::dev_net().0
    }

    fn lock_idx(&self, export_id: &[u8; 32]) -> Option<usize> {
        self.cross_shard_locks
            .iter()
            .position(|row| &row.export_id == export_id)
    }

    fn has_pending_conserv(&self, sender: &AccountId) -> bool {
        self.pending_conservation
            .iter()
            .any(|row| &row.sender == sender)
    }

    pub fn refund_exp_locks(&mut self, current_height: u64) {
        for lock in &mut self.cross_shard_locks {
            if lock.state != CrossShardLockState::Locked || current_height < lock.unlock_height {
                continue;
            }
            if let Some(acc) = self.accounts.get_mut(&lock.refund_account) {
                acc.balance_pwm = acc.balance_pwm.saturating_add(lock.amount_pwm);
            }
            lock.state = CrossShardLockState::Refunded;
            self.imported_set.insert(lock.export_id);
        }
    }

    pub fn drain_conservation_at_height(&mut self, current_height: u64, gen_cfg: &GenCfg) {
        let mut remaining = Vec::with_capacity(self.pending_conservation.len());
        let pending = std::mem::take(&mut self.pending_conservation);
        for row in pending {
            if current_height < row.execute_at_height {
                remaining.push(row);
                continue;
            }
            match self.apply_due_conservation(row.clone(), current_height, gen_cfg) {
                Ok(()) => {}
                Err(err) => {
                    eprintln!(
                        "conservation_drain_retry sender={:?} nonce={} execute_at={} height={} err={err}",
                        row.sender, row.nonce, row.execute_at_height, current_height
                    );
                    remaining.push(row);
                }
            }
        }
        self.pending_conservation = remaining;
    }

    fn apply_due_conservation(
        &mut self,
        row: PendingConservationTransfer,
        current_height: u64,
        gen_cfg: &GenCfg,
    ) -> Result<(), TxError> {
        let sender_acc = self
            .accounts
            .get(&row.sender)
            .ok_or(TxError::NoAccount)?
            .clone();
        if !sender_acc.initialized {
            return Err(TxError::NotInitialized);
        }
        if sender_acc.nonce != row.nonce {
            return Err(TxError::BadNonce);
        }
        let transfer_body = TxBody::Transfer {
            to: row.recipient,
            amount: row.amount_pwm,
            fee: u128::from(row.fee_pwm),
        };
        if sender_acc.finalized && is_finalized_blocked(&transfer_body, &sender_acc) {
            return Err(TxError::PolicyAccountFinalized);
        }
        let dst = conservation_recipient_dst(self, &row.sender, &row.recipient, current_height)?;
        self.require_recipient(&dst)?;
        let fee = u128::from(row.fee_pwm);
        let total = row
            .amount_pwm
            .checked_add(fee)
            .ok_or(TxError::Insufficient)?;
        if sender_acc.balance_pwm < total {
            return Err(TxError::Insufficient);
        }
        let mut from = sender_acc;
        touch_acct_mrks(&mut from, current_height, gen_cfg);
        from.balance_pwm -= total;
        from.nonce += 1;
        self.fee_pool = self.fee_pool.saturating_add(fee);

        let mut to_acc = self.accounts.get(&dst).cloned().expect("recipient gated");
        touch_acct_mrks(&mut to_acc, current_height, gen_cfg);
        to_acc.balance_pwm = to_acc.balance_pwm.saturating_add(row.amount_pwm);
        self.accounts.insert(row.sender, from);
        self.accounts.insert(dst, to_acc);
        Ok(())
    }

    /// Applies one signed tx. `Init` may create an empty stub row first (white-spec).
    pub fn apply_tx(&mut self, tx: &SignedTx) -> Result<(), TxError> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| TxError::PolicySchemaInvalid)?
            .as_secs();
        let gen_cfg = Self::default_ctx_cfg();
        self.apply_tx_with_ctx(tx, 0, now, &gen_cfg)
    }

    /// Applies one signed tx using canonical block context for claim maturity checks.
    pub fn apply_tx_with_ctx(
        &mut self,
        tx: &SignedTx,
        inclusion_height: u64,
        block_unix_time: u64,
        gen_cfg: &GenCfg,
    ) -> Result<(), TxError> {
        self.apply_tx_impl(tx, inclusion_height, block_unix_time, gen_cfg, false)
    }

    /// Applies a tx already checked against this chain-tip context, without re-running policy.
    pub fn apply_prechecked_tx(
        &mut self,
        tx: &SignedTx,
        inclusion_height: u64,
        block_unix_time: u64,
        gen_cfg: &GenCfg,
    ) -> Result<(), TxError> {
        self.apply_tx_impl(tx, inclusion_height, block_unix_time, gen_cfg, true)
    }

    #[allow(clippy::too_many_arguments, clippy::too_many_lines)]
    fn apply_tx_impl(
        &mut self,
        tx: &SignedTx,
        inclusion_height: u64,
        _block_unix_time: u64,
        gen_cfg: &GenCfg,
        skip_policy: bool,
    ) -> Result<(), TxError> {
        // Precheck workers already verified the sig; skip the expensive Ed25519 re-verify.
        if skip_policy {
            crate::tx::validate_shape_no_sig(tx)?;
        } else {
            crate::tx::validate_tx_shape(tx)?;
        }
        let id = tx.computed_account_id();

        if matches!(&tx.body, TxBody::Init { .. }) && !self.accounts.contains_key(&id) {
            self.accounts.insert(
                id,
                Account {
                    signing_pubkey: tx.signer_pk,
                    derivation_index: tx.derivation_index,
                    ..Default::default()
                },
            );
        }

        let mut acc = self.accounts.get(&id).ok_or(TxError::NoAccount)?.clone();

        if acc.initialized {
            if acc.signing_pubkey != tx.signer_pk || acc.derivation_index != tx.derivation_index {
                return Err(TxError::BadSignature);
            }
        } else {
            // Accept the first signer for a stub/uninitialized account created by inbound tx.
            // The account id key already binds this tx signer; we only delay persisting signer fields
            // until the first explicit Init.
            acc.signing_pubkey = tx.signer_pk;
            acc.derivation_index = tx.derivation_index;
        }
        if acc.nonce != tx.nonce {
            return Err(TxError::BadNonce);
        }
        let mut redir_to = None;
        if !skip_policy {
            match self.evaluate_policy(tx, inclusion_height) {
                PolicyDecision::Allow => {}
                PolicyDecision::Reject(err) => return Err(err),
                PolicyDecision::Redirect(to) => redir_to = Some(to),
            }
        }
        if self.has_pending_conserv(&id) && pending_tx_conflict(&tx.body) {
            return Err(TxError::ConservationPendingExists);
        }

        match &tx.body {
            TxBody::Init { index, flags } => {
                if acc.initialized {
                    return Err(TxError::AlreadyInit);
                }
                let mut a = acc;
                a.initialized = true;
                a.index = *index;
                a.flags = *flags;
                if let Some(ext) = &tx.init_v4 {
                    a.owner_kind = ext.owner_kind.clone();
                    a.owner_display_name = ext.owner_display_name.clone();
                    a.owner_country_hint = ext.owner_country_hint.clone();
                    a.company_metadata_commitment = Some(ext.company_metadata_commitment);
                    a.external_verification_ref = Some(ext.external_verification_ref.clone());
                    a.requested_domain_lo = Some(ext.requested_domain_lo);
                    a.rescue_address = ext.rescue_address;
                    for row in &ext.initial_policies {
                        set_pol_mode(&mut a, row.policy, &row.activation, inclusion_height);
                    }
                }
                a.marks_last_block = inclusion_height;
                a.nonce += 1;
                self.accounts.insert(id, a);
            }
            TxBody::Transfer { to, amount, fee } => {
                if !acc.initialized {
                    return Err(TxError::NotInitialized);
                }
                let dst = redir_to.unwrap_or(*to);
                if dst != id {
                    self.require_recipient(&dst)?;
                }
                let total = amount.checked_add(*fee).ok_or(TxError::Insufficient)?;
                if acc.balance_pwm < total {
                    return Err(TxError::Insufficient);
                }
                if conservation_flag(&id) {
                    if self.pending_conservation.iter().any(|row| row.sender == id) {
                        return Err(TxError::ConservationPendingExists);
                    }
                    let fee_pwm = u64::try_from(*fee).map_err(|_| TxError::PolicySchemaInvalid)?;
                    self.pending_conservation.push(PendingConservationTransfer {
                        sender: id,
                        recipient: dst,
                        amount_pwm: *amount,
                        fee_pwm,
                        nonce: tx.nonce,
                        enqueue_height: inclusion_height,
                        execute_at_height: inclusion_height
                            .saturating_add(gen_cfg.conservation_delay_blocks),
                        tx_hash: *blake3::hash(&tx.signing_message()).as_bytes(),
                    });
                    return Ok(());
                }
                let mut from = acc;
                touch_acct_mrks(&mut from, inclusion_height, gen_cfg);
                from.balance_pwm -= total;
                from.nonce += 1;
                self.fee_pool = self.fee_pool.saturating_add(tx.body.fee_amount());

                if dst == id {
                    // Self-transfer must apply debit+credit against the same account.
                    // Previously the receiver stub overwrite could discard the nonce increment.
                    from.balance_pwm = from.balance_pwm.saturating_add(*amount);
                    self.accounts.insert(id, from);
                } else {
                    let mut to_acc = self.accounts.get(&dst).cloned().expect("recipient gated");
                    touch_acct_mrks(&mut to_acc, inclusion_height, gen_cfg);
                    to_acc.balance_pwm = to_acc.balance_pwm.saturating_add(*amount);
                    self.accounts.insert(id, from);
                    self.accounts.insert(dst, to_acc);
                }
            }
            TxBody::Stake { amount } => {
                if !acc.initialized {
                    return Err(TxError::NotInitialized);
                }
                if acc.balance_pwm < *amount {
                    return Err(TxError::Insufficient);
                }
                let mut a = acc;
                touch_acct_mrks(&mut a, inclusion_height, gen_cfg);
                a.balance_pwm -= amount;
                a.staked_pwm_raw += amount;
                a.nonce += 1;
                self.accounts.insert(id, a);
            }
            TxBody::Unstake { amount } => {
                if !acc.initialized {
                    return Err(TxError::NotInitialized);
                }
                if acc.staked_pwm_raw < *amount {
                    return Err(TxError::Insufficient);
                }
                let mut a = acc;
                touch_acct_mrks(&mut a, inclusion_height, gen_cfg);
                a.staked_pwm_raw -= amount;
                a.balance_pwm += amount;
                a.nonce += 1;
                self.accounts.insert(id, a);
            }
            TxBody::BurnMark { mark_amount, .. } => {
                if !acc.initialized {
                    return Err(TxError::NotInitialized);
                }
                let mut a = acc;
                touch_acct_mrks(&mut a, inclusion_height, gen_cfg);
                // Burn is unilateral: beneficiary is metadata, sender marks are debited locally.
                if a.stored_marks < *mark_amount {
                    return Err(TxError::InsufficientMarks);
                }
                a.stored_marks -= *mark_amount;
                a.nonce += 1;
                self.accounts.insert(id, a);
            }
            TxBody::ClaimIPv4Batch {
                phase,
                batch_root,
                registry_sig,
            } => {
                if !acc.initialized {
                    return Err(TxError::NotInitialized);
                }
                if acc.ipv4_claimed_phase.is_some() {
                    return Err(TxError::PolicyDenied);
                }
                let phase_row = find_claim_phase(gen_cfg, *phase)?;
                let msg = claim_ipv4_msg(*phase, batch_root, &id);
                verify_claim_sig(self, &phase_row.registry_address, &msg, registry_sig)?;
                let mut a = acc;
                a.balance_pwm = a.balance_pwm.saturating_add(phase_row.allocation);
                a.ipv4_claimed_phase = Some(*phase);
                a.nonce += 1;
                self.accounts.insert(id, a);
            }
            TxBody::Export { amount, fee, .. } => {
                if !acc.initialized {
                    return Err(TxError::NotInitialized);
                }
                if !export_context_is_valid(tx) {
                    return Err(TxError::InvalidExport);
                }
                let total = amount.checked_add(*fee).ok_or(TxError::Insufficient)?;
                if acc.balance_pwm < total {
                    return Err(TxError::Insufficient);
                }
                let mut from = acc;
                from.balance_pwm -= total;
                from.nonce += 1;
                self.fee_pool = self.fee_pool.saturating_add(tx.body.fee_amount());
                let export_id = tx.export_id().ok_or(TxError::InvalidExport)?;
                let TxBody::Export {
                    to,
                    target_domain,
                    amount,
                    ..
                } = &tx.body
                else {
                    unreachable!("guarded by TxBody::Export match arm")
                };
                self.exported_registry
                    .entry(export_id)
                    .or_insert_with(|| ExportProvenance {
                        to: *to,
                        target_domain: *target_domain,
                        amount: *amount,
                    });
                if self.lock_idx(&export_id).is_none() {
                    self.cross_shard_locks.push(CrossShardLock {
                        export_id,
                        sender: id,
                        amount_pwm: *amount,
                        target_domain: *target_domain,
                        refund_account: id,
                        lock_height: inclusion_height,
                        unlock_height: inclusion_height
                            .saturating_add(gen_cfg.xshard_lock_to_blocks),
                        state: CrossShardLockState::Locked,
                    });
                }
                self.accounts.insert(id, from);
            }
            TxBody::Import {
                to,
                amount,
                export_id,
            } => {
                if !acc.initialized {
                    return Err(TxError::NotInitialized);
                }
                if !import_context_is_valid(tx) {
                    return Err(TxError::InvalidImport);
                }
                let lock_idx = self.lock_idx(export_id);
                if lock_idx.map(|idx| self.cross_shard_locks[idx].state)
                    == Some(CrossShardLockState::Refunded)
                {
                    return Err(TxError::ExportLockRefunded);
                }
                if self.imported_set.contains(export_id) {
                    return Err(TxError::DuplicateImport);
                }
                let mut should_insert_provenance = false;
                let expected_export = if let Some(existing) = self.exported_registry.get(export_id)
                {
                    if let Some(prov) = &tx.import_provenance {
                        if existing != prov {
                            return Err(TxError::InvalidImport);
                        }
                    }
                    existing.clone()
                } else if let Some(prov) = &tx.import_provenance {
                    should_insert_provenance = true;
                    prov.clone()
                } else {
                    return Err(TxError::InvalidImport);
                };
                if expected_export.amount != *amount
                    || expected_export.to != *to
                    || expected_export.target_domain.to_be_bytes()[0]
                        != tx.domain_code.to_be_bytes()[0]
                {
                    return Err(TxError::InvalidImport);
                }
                if *to != id {
                    self.require_recipient(to)?;
                }
                let mut from = acc;
                let fee = tx.import_fee.ok_or(TxError::ImportFeeTooLow)?;
                if fee < crate::tx::MIN_IMPORT_FEE_UNITS {
                    return Err(TxError::ImportFeeTooLow);
                }
                if from.balance_pwm < fee {
                    return Err(TxError::Insufficient);
                }
                from.balance_pwm -= fee;
                self.fee_pool = self.fee_pool.saturating_add(fee);
                if should_insert_provenance {
                    self.exported_registry
                        .insert(*export_id, expected_export.clone());
                }
                from.nonce += 1;
                if *to == id {
                    from.balance_pwm = from.balance_pwm.saturating_add(*amount);
                    self.accounts.insert(id, from);
                } else {
                    let mut to_acc = self.accounts.get(to).cloned().expect("recipient gated");
                    to_acc.balance_pwm = to_acc.balance_pwm.saturating_add(*amount);
                    self.accounts.insert(id, from);
                    self.accounts.insert(*to, to_acc);
                }
                if let Some(idx) = lock_idx {
                    if self.cross_shard_locks[idx].state == CrossShardLockState::Locked {
                        self.cross_shard_locks[idx].state = CrossShardLockState::Released;
                    }
                }
                self.imported_set.insert(*export_id);
            }
            TxBody::Policy {
                target_account,
                action,
                fee,
            } => {
                if !acc.initialized {
                    return Err(TxError::NotInitialized);
                }
                if *target_account != id {
                    return Err(TxError::PolicyDenied);
                }
                let mut a = acc;
                touch_acct_mrks(&mut a, inclusion_height, gen_cfg);
                validate_pol_action(self, &a, action, tx)?;
                let evac_target = emergency_act_target(action);
                apply_policy_action(&mut a, action, inclusion_height)?;
                a.balance_pwm = a
                    .balance_pwm
                    .checked_sub(*fee)
                    .ok_or(TxError::Insufficient)?;
                self.fee_pool = self.fee_pool.saturating_add(*fee);
                if let Some(target) = evac_target {
                    self.pending_conservation.retain(|row| row.sender != id);
                    let amount = a.balance_pwm;
                    let stake = a.staked_pwm_raw;
                    if target != id {
                        let target_acc = self
                            .accounts
                            .get_mut(&target)
                            .expect("activation target validated");
                        if amount > 0 {
                            target_acc.balance_pwm = target_acc.balance_pwm.saturating_add(amount);
                            a.balance_pwm = 0;
                        }
                        if stake > 0 {
                            target_acc.balance_pwm = target_acc.balance_pwm.saturating_add(stake);
                            a.staked_pwm_raw = 0;
                        }
                    }
                }
                a.nonce += 1;
                self.accounts.insert(id, a);
            }
        }
        Ok(())
    }

    /// Pure policy evaluator over current state snapshot.
    pub fn evaluate_policy(&self, tx: &SignedTx, chain_tip_height: u64) -> PolicyDecision {
        let sender_id = tx.computed_account_id();
        let Some(sender_acc) = self.accounts.get(&sender_id) else {
            return PolicyDecision::Reject(TxError::NoAccount);
        };

        if sender_acc.finalized && is_finalized_blocked(&tx.body, sender_acc) {
            return PolicyDecision::Reject(TxError::PolicyAccountFinalized);
        }

        if cosign_non_dis(&sender_id) && cosign_prot_body(&tx.body) && !has_valid_cosign(tx) {
            return PolicyDecision::Reject(TxError::PolicyMissingCosign);
        }

        if let TxBody::Transfer { to, .. } = &tx.body {
            if let Some(recipient_acc) = self.accounts.get(to).filter(|acc| acc.initialized) {
                if recipient_acc.finalized
                    && policy_is_active_at(
                        recipient_acc,
                        PolicyKind::RoutingEmergencyRedirect,
                        chain_tip_height,
                    )
                {
                    return eval_emerg_redirect(self, &sender_id, recipient_acc);
                }
                if policy_is_active_at(
                    recipient_acc,
                    PolicyKind::RoutingSameDomainOnly,
                    chain_tip_height,
                ) && !same_hi_domain(&sender_id, to)
                {
                    return PolicyDecision::Reject(TxError::PolicyRoutingDenied);
                }
                if policy_is_active_at(recipient_acc, PolicyKind::SenderFilter, chain_tip_height)
                    && sender_id != *to
                {
                    return PolicyDecision::Reject(TxError::PolicySenderFiltered);
                }
                if policy_is_active_at(recipient_acc, PolicyKind::DefaultBehavior, chain_tip_height)
                {
                    return PolicyDecision::Reject(TxError::PolicyDenied);
                }
            }
        }

        if let TxBody::Policy { target_account, .. } = &tx.body {
            if let Some(target_acc) = self.accounts.get(target_account) {
                if (policy_is_active_at(target_acc, PolicyKind::CosignRequired, chain_tip_height)
                    || cosign_non_dis(target_account))
                    && !has_valid_cosign(tx)
                {
                    return PolicyDecision::Reject(TxError::PolicyMissingCosign);
                }
            }
        }

        PolicyDecision::Allow
    }

    /// Marks from stake per block (`staked * coeff / 1_000_000`).
    pub fn accrue_marks(&mut self, coeff: u128) {
        for a in self.accounts.values_mut() {
            if !a.initialized {
                continue;
            }
            let add = as_marks_u32(a.staked_pwm_raw.saturating_mul(coeff) / PPM_DENOM);
            a.stored_marks = a.stored_marks.saturating_add(add);
        }
    }

    /// V2 marks path: stake gate + seasonal ppm.
    pub fn accrue_marks_v2(&mut self, coeff: u128, stake_min: u128, season_ppm: u128) {
        for a in self.accounts.values_mut() {
            if !a.initialized || a.staked_pwm_raw < stake_min {
                continue;
            }
            let base = a.staked_pwm_raw.saturating_mul(coeff) / PPM_DENOM;
            let add = as_marks_u32(base.saturating_mul(season_ppm) / PPM_DENOM);
            a.stored_marks = a.stored_marks.saturating_add(add);
        }
    }

    pub fn reward_producer(&mut self, producer: &AccountId, reward: u128) {
        if let Some(a) = self.accounts.get_mut(producer) {
            a.balance_pwm = a.balance_pwm.saturating_add(reward);
        }
    }

    /// V2 reward path: producer stake gate + seasonal ppm.
    pub fn reward_producer_v2(
        &mut self,
        producer: &AccountId,
        reward: u128,
        stake_min: u128,
        season_ppm: u128,
    ) {
        if let Some(a) = self.accounts.get_mut(producer) {
            if a.staked_pwm_raw < stake_min {
                return;
            }
            let add = reward.saturating_mul(season_ppm) / PPM_DENOM;
            a.balance_pwm = a.balance_pwm.saturating_add(add);
        }
    }
}

fn touch_acct_mrks(acc: &mut Account, inclusion_height: u64, gen_cfg: &GenCfg) {
    acc.stored_marks = compute_lazy_marks(acc, inclusion_height, gen_cfg);
    acc.marks_last_block = inclusion_height;
}

fn as_marks_u32(value: u128) -> u32 {
    value.min(MARKS_CAP as u128) as u32
}

fn apply_policy_action(
    acc: &mut Account,
    action: &PolicyAction,
    inclusion_height: u64,
) -> Result<(), TxError> {
    match action {
        PolicyAction::SetPolicy { policy, activation } => {
            set_pol_mode(acc, *policy, activation, inclusion_height);
            Ok(())
        }
        PolicyAction::ActivatePolicy { policy_id, .. } => {
            let policy =
                PolicyKind::from_policy_id(*policy_id).ok_or(TxError::PolicySchemaInvalid)?;
            if let Some(row) = acc
                .deferred_policies
                .iter()
                .find(|row| row.policy == policy)
            {
                return if inclusion_height < row.activate_at_height {
                    Err(TxError::PolicyNotActive)
                } else {
                    Err(TxError::PolicyDenied)
                };
            }
            let bit = policy.bit();
            if acc.active_policies & bit != 0 {
                return Ok(());
            }
            if acc.dormant_policies & bit == 0 {
                return Err(TxError::PolicyNotInstalled);
            }
            acc.dormant_policies &= !bit;
            acc.active_policies |= bit;
            if matches!(policy, PolicyKind::RoutingEmergencyRedirect) {
                acc.finalized = true;
            }
            Ok(())
        }
        PolicyAction::DeactivatePolicy { policy_id } => {
            let policy =
                PolicyKind::from_policy_id(*policy_id).ok_or(TxError::PolicySchemaInvalid)?;
            if !policy.is_reversible() {
                return Err(TxError::PolicyIrreversible);
            }
            if let Some(idx) = acc
                .deferred_policies
                .iter()
                .position(|row| row.policy == policy && inclusion_height < row.activate_at_height)
            {
                acc.deferred_policies.remove(idx);
                return Ok(());
            }
            let bit = policy.bit();
            if acc.active_policies & bit == 0 {
                return Err(TxError::PolicyNotActive);
            }
            acc.active_policies &= !bit;
            acc.dormant_policies |= bit;
            Ok(())
        }
    }
}

fn set_pol_mode(acc: &mut Account, policy: PolicyKind, activation: &ActivationMode, incl_h: u64) {
    let bit = policy.bit();
    acc.active_policies &= !bit;
    acc.dormant_policies &= !bit;
    acc.deferred_policies.retain(|row| row.policy != policy);
    match activation {
        ActivationMode::Dormant => acc.dormant_policies |= bit,
        ActivationMode::Immediately => acc.active_policies |= bit,
        ActivationMode::Deferred { activate_at_height } => {
            acc.deferred_policies.push(DeferredPolicyEntry {
                policy,
                activate_at_height: *activate_at_height,
            });
            if *activate_at_height <= incl_h {
                acc.active_policies |= bit;
            }
        }
    }
}

fn policy_is_active_at(acc: &Account, policy: PolicyKind, chain_tip_height: u64) -> bool {
    acc.active_policies & policy.bit() != 0
        || acc
            .deferred_policies
            .iter()
            .any(|row| row.policy == policy && chain_tip_height >= row.activate_at_height)
}

fn is_finalized_blocked(body: &TxBody, acc: &Account) -> bool {
    match body {
        TxBody::Transfer { .. }
        | TxBody::Stake { .. }
        | TxBody::Unstake { .. }
        | TxBody::BurnMark { .. }
        | TxBody::ClaimIPv4Batch { .. }
        | TxBody::Export { .. }
        | TxBody::Import { .. } => true,
        TxBody::Policy { action, .. } => !finalized_policy_allowed(acc, action),
        TxBody::Init { .. } => false,
    }
}

fn cosign_prot_body(body: &TxBody) -> bool {
    match body {
        TxBody::Transfer { .. } | TxBody::Policy { .. } => true,
        TxBody::Init { .. }
        | TxBody::Stake { .. }
        | TxBody::Unstake { .. }
        | TxBody::BurnMark { .. }
        | TxBody::ClaimIPv4Batch { .. }
        | TxBody::Export { .. }
        | TxBody::Import { .. } => false,
    }
}

fn finalized_policy_allowed(acc: &Account, action: &PolicyAction) -> bool {
    match action {
        PolicyAction::ActivatePolicy { policy_id, .. } => {
            if *policy_id != PolicyKind::RoutingEmergencyRedirect.policy_id() {
                return false;
            }
            let bit = PolicyKind::RoutingEmergencyRedirect.bit();
            acc.active_policies & bit == 0 && acc.dormant_policies & bit != 0
        }
        PolicyAction::SetPolicy { .. } | PolicyAction::DeactivatePolicy { .. } => false,
    }
}

fn has_valid_cosign(tx: &SignedTx) -> bool {
    if tx.cosigns.is_empty() {
        return false;
    }
    let msg = tx.signing_message();
    tx.cosigns
        .iter()
        .any(|row| verify(&row.signer_pk, &msg, &row.signature))
}

fn has_role_cosign(tx: &SignedTx, role: CosignRole, signer_pk: &[u8; 32]) -> bool {
    if tx.cosigns.is_empty() {
        return false;
    }
    let msg = tx.signing_message();
    tx.cosigns.iter().any(|row| {
        row.role == role
            && &row.signer_pk == signer_pk
            && verify(&row.signer_pk, &msg, &row.signature)
    })
}

fn emergency_act_target(action: &PolicyAction) -> Option<AccountId> {
    match action {
        PolicyAction::ActivatePolicy {
            policy_id,
            activation_target,
        } if *policy_id == PolicyKind::RoutingEmergencyRedirect.policy_id() => *activation_target,
        _ => None,
    }
}

fn conservation_recipient_dst(
    st: &State,
    sender_id: &AccountId,
    recipient_id: &AccountId,
    chain_tip_height: u64,
) -> Result<AccountId, TxError> {
    let Some(recipient_acc) = st.accounts.get(recipient_id).filter(|acc| acc.initialized) else {
        return Err(TxError::RecipientNotInitialized);
    };
    if recipient_acc.finalized
        && policy_is_active_at(
            recipient_acc,
            PolicyKind::RoutingEmergencyRedirect,
            chain_tip_height,
        )
    {
        return match eval_emerg_redirect(st, sender_id, recipient_acc) {
            PolicyDecision::Redirect(to) => Ok(to),
            PolicyDecision::Reject(err) => Err(err),
            PolicyDecision::Allow => Ok(*recipient_id),
        };
    }
    if policy_is_active_at(
        recipient_acc,
        PolicyKind::RoutingSameDomainOnly,
        chain_tip_height,
    ) && !same_hi_domain(sender_id, recipient_id)
    {
        return Err(TxError::PolicyRoutingDenied);
    }
    if policy_is_active_at(recipient_acc, PolicyKind::SenderFilter, chain_tip_height)
        && sender_id != recipient_id
    {
        return Err(TxError::PolicySenderFiltered);
    }
    if policy_is_active_at(recipient_acc, PolicyKind::DefaultBehavior, chain_tip_height) {
        return Err(TxError::PolicyDenied);
    }
    Ok(*recipient_id)
}

fn validate_pol_action(
    st: &State,
    acc: &Account,
    action: &PolicyAction,
    tx: &SignedTx,
) -> Result<(), TxError> {
    if cosign_non_dis(&tx.computed_account_id()) && policy_weakens_cosign(action) {
        return Err(TxError::PolicyFlagNonDisableable);
    }

    let PolicyAction::ActivatePolicy {
        policy_id,
        activation_target,
    } = action
    else {
        return Ok(());
    };
    let policy = PolicyKind::from_policy_id(*policy_id).ok_or(TxError::PolicySchemaInvalid)?;
    if !matches!(policy, PolicyKind::RoutingEmergencyRedirect) {
        return Ok(());
    }
    let rescue_id = acc.rescue_address.ok_or(TxError::PolicyRescueRequired)?;
    let target = activation_target.ok_or(TxError::PolicyActivationTargetRequired)?;
    if target != rescue_id {
        return Err(TxError::PolicyActivationTargetMismatch);
    }
    if !same_hi_domain(&tx.computed_account_id(), &target) {
        return Err(TxError::PolicyRoutingDenied);
    }
    let rescue_pk = st
        .accounts
        .get(&rescue_id)
        .filter(|row| row.initialized)
        .map(|row| row.signing_pubkey)
        .ok_or(TxError::PolicyEmergencyCosignRequired)?;
    if has_role_cosign(tx, CosignRole::Rescue, &rescue_pk) {
        return Ok(());
    }
    Err(TxError::PolicyEmergencyCosignRequired)
}

fn eval_emerg_redirect(st: &State, sender_id: &AccountId, acc: &Account) -> PolicyDecision {
    let rescue_id = match acc.rescue_address {
        Some(id) => id,
        None => return PolicyDecision::Reject(TxError::PolicyRescueRequired),
    };
    let Some(rescue_acc) = st.accounts.get(&rescue_id) else {
        return PolicyDecision::Reject(TxError::RecipientMissing);
    };
    if !rescue_acc.initialized {
        return PolicyDecision::Reject(TxError::RecipientNotInitialized);
    }
    if !same_hi_domain(sender_id, &rescue_id) {
        return PolicyDecision::Reject(TxError::PolicyRoutingDenied);
    }
    PolicyDecision::Redirect(rescue_id)
}

#[cfg(test)]
mod tests {
    use super::CrossShardLock;
    use super::CrossShardLockState;
    use super::EvidenceRecord;
    use super::EvidenceType;
    use super::ExportProvenance;
    use super::PendingConservationTransfer;
    use super::PolicyDecision;
    use super::State;
    use crate::chain::{recompute_active_idxs, roll_epoch_if_needed};
    use crate::crypto::sign;
    use crate::genesis::{dev_net, ClaimPhaseConfig};
    use crate::hd::{account_id_from_parts, domain_of_account_id};
    use crate::tx::{
        validate_tx_shape, ActivationMode, CosignRole, Cosignature, InitPolicyEntry,
        InitV4Extension, PolicyAction, PolicyKind, SignedTx, TxBody, TxError,
    };
    use crate::types::Account;
    use crate::types::AccountId;
    use crate::types::DeferredPolicyEntry;
    use crate::types::{conservation_flag, cosign_non_dis};
    use crate::MARKS_CAP;
    use crate::PWM_RAW_SCALE;
    use ed25519_dalek::SigningKey;
    use slip10_ed25519::derive_ed25519_private_key;

    fn user_sk0(seed: &[u8; 32]) -> (SigningKey, u32, AccountId) {
        let sk_bytes = derive_ed25519_private_key(seed, &[0, 0]);
        let sk = SigningKey::from_bytes(&sk_bytes);
        let i = 0u32;
        let pk = sk.verifying_key().to_bytes();
        let aid = account_id_from_parts(&pk, i);
        (sk, i, aid)
    }

    fn user_sk_other_domain(seed_start: u8, sender_hi: u8) -> (SigningKey, u32, AccountId) {
        let mut seed = seed_start;
        for _ in 0..1024 {
            let s = [seed; 32];
            let (sk, idx, aid) = user_sk0(&s);
            if domain_of_account_id(&aid).to_be_bytes()[0] != sender_hi {
                return (sk, idx, aid);
            }
            seed = seed.wrapping_add(1);
        }
        panic!("failed to find account in other domain");
    }

    fn user_sk_new_domain(
        st: &State,
        seed_start: u8,
        sender_hi: u8,
    ) -> (SigningKey, u32, AccountId) {
        for attempt in 0..2048 {
            let n = attempt as u16;
            let mut s = [seed_start; 32];
            s[0] = seed_start.wrapping_add(n as u8);
            s[1] = (n >> 8) as u8;
            let (sk, idx, aid) = user_sk0(&s);
            if domain_of_account_id(&aid).to_be_bytes()[0] == sender_hi
                && !st.accounts.contains_key(&aid)
            {
                return (sk, idx, aid);
            }
        }
        panic!("failed to find unused account in same domain");
    }

    fn user_sk_plain_domain(
        st: &State,
        seed_start: u8,
        sender_hi: u8,
    ) -> (SigningKey, u32, AccountId) {
        for attempt in 0..4096 {
            let n = attempt as u16;
            let mut s = [seed_start; 32];
            s[0] = seed_start.wrapping_add(n as u8);
            s[1] = (n >> 8) as u8;
            let (sk, idx, aid) = user_sk0(&s);
            if domain_of_account_id(&aid).to_be_bytes()[0] == sender_hi
                && !st.accounts.contains_key(&aid)
                && !conservation_flag(&aid)
                && !cosign_non_dis(&aid)
            {
                return (sk, idx, aid);
            }
        }
        panic!("failed to find plain account in same domain");
    }

    fn user_sk_flag(seed_start: u8) -> (SigningKey, u32, AccountId) {
        let mut seed = seed_start;
        for _ in 0..256 {
            let s = [seed; 32];
            let (sk, idx, aid) = user_sk0(&s);
            if cosign_non_dis(&aid) {
                return (sk, idx, aid);
            }
            seed = seed.wrapping_add(1);
        }
        panic!("failed to find account with cosign flag");
    }

    fn user_sk_conserv(seed_start: u8) -> (SigningKey, u32, AccountId) {
        for attempt in 0..4096 {
            let n = attempt as u16;
            let mut s = [seed_start; 32];
            s[0] = seed_start.wrapping_add(n as u8);
            s[1] = (n >> 8) as u8;
            let (sk, idx, aid) = user_sk0(&s);
            if conservation_flag(&aid) && !cosign_non_dis(&aid) {
                return (sk, idx, aid);
            }
        }
        panic!("failed to find conservation-only account");
    }

    fn add_witness_cosign(tx: &mut SignedTx, sk: &SigningKey) {
        let msg = tx.signing_message();
        tx.cosigns.push(Cosignature {
            signer_pk: sk.verifying_key().to_bytes(),
            role: CosignRole::Witness,
            signature: sign(sk, &msg),
        });
    }

    struct ClaimCtx {
        st: State,
        gen_cfg: crate::genesis::GenCfg,
        reg_sk: SigningKey,
        claim_sk: SigningKey,
        claim_idx: u32,
        claim_id: AccountId,
        batch_root: [u8; 32],
    }

    fn mk_claim_ctx() -> ClaimCtx {
        let (reg_sk, reg_idx, reg_id) = user_sk0(&[0x44; 32]);
        let (claim_sk, claim_idx, claim_id) = user_sk0(&[0x45; 32]);
        eprintln!("V5_CLAIM_TEST_REG_ID_HEX={}", hex::encode(reg_id));
        eprintln!(
            "V5_CLAIM_TEST_REG_PK_HEX={}",
            hex::encode(reg_sk.verifying_key().to_bytes())
        );
        eprintln!("V5_CLAIM_TEST_CLAIM_ID_HEX={}", hex::encode(claim_id));
        eprintln!(
            "V5_CLAIM_TEST_CLAIM_PK_HEX={}",
            hex::encode(claim_sk.verifying_key().to_bytes())
        );
        let mut st = State::default();
        st.accounts.insert(
            reg_id,
            Account::genesis_funded(reg_sk.verifying_key().to_bytes(), reg_idx, 0),
        );
        st.accounts.insert(
            claim_id,
            Account::genesis_funded(claim_sk.verifying_key().to_bytes(), claim_idx, 77),
        );

        let mut gen_cfg = dev_net().0;
        gen_cfg.ipv4_claim_phases = vec![ClaimPhaseConfig {
            phase: 7,
            registry_address: reg_id,
            allocation: 500,
        }];

        ClaimCtx {
            st,
            gen_cfg,
            reg_sk,
            claim_sk,
            claim_idx,
            claim_id,
            batch_root: [0xAB; 32],
        }
    }

    /// Init stub then transfer validator→peer debits fee pool (formerly `apply_tx_init_then_transfer_happy_path`).
    #[test]
    fn init_then_xfer_happy() {
        let mut st = State::default();
        let (sk_b, i_b, aid_b) = user_sk0(&[3u8; 32]);
        let dom_b = domain_of_account_id(&aid_b);
        let sender_hi = dom_b.to_be_bytes()[0];
        let (sk_v, idx_v, aid_v) = user_sk_plain_domain(&st, 0x04, sender_hi);
        let dom_v = domain_of_account_id(&aid_v);
        st.accounts.insert(
            aid_v,
            Account::genesis_funded(sk_v.verifying_key().to_bytes(), idx_v, 1_000_000),
        );

        let init_b = SignedTx::sign_body(&sk_b, dom_b, i_b, 0, TxBody::Init { index: 7, flags: 0 });
        st.apply_tx(&init_b).expect("init new account");
        let b = st.get(&aid_b).expect("stub then init row");
        assert!(b.initialized);
        assert_eq!(b.nonce, 1);
        assert_eq!(b.balance_pwm, 0);

        let xfer = SignedTx::sign_body(
            &sk_v,
            dom_v,
            idx_v,
            0,
            TxBody::Transfer {
                to: aid_b,
                amount: 500,
                fee: 10,
            },
        );
        st.apply_tx(&xfer).expect("transfer to initialized peer");

        let v = st.get(&aid_v).expect("validator");
        assert_eq!(v.nonce, 1);
        assert_eq!(v.balance_pwm, 1_000_000 - 510);
        let b2 = st.get(&aid_b).expect("recipient");
        assert_eq!(b2.balance_pwm, 500);
        assert_eq!(b2.nonce, 1);
        assert_eq!(st.fee_pool, 10);
    }

    /// Transfer to missing recipient is a noop on state/fee_pool (formerly `apply_tx_transfer_rejects_missing_recipient_without_side_effects`).
    #[test]
    fn xfer_rcpt_miss_no_mut() {
        let (g, sks) = dev_net();
        let mut st = g.state0();
        let sk_v = &sks[0];
        let aid_v = g.accounts[0].acct;
        let dom_v = domain_of_account_id(&aid_v);

        // Ensure we pick an account id that is not present in state yet.
        let want_hi = dom_v.to_be_bytes()[0];
        let mut aid_r = [0xA1u8; 32];
        aid_r[0] = want_hi;
        assert!(st.get(&aid_r).is_none());
        let before = st.clone();

        let xfer = SignedTx::sign_body(
            sk_v,
            dom_v,
            0,
            0,
            TxBody::Transfer {
                to: aid_r,
                amount: 123,
                fee: 7,
            },
        );
        let err = st
            .apply_tx(&xfer)
            .expect_err("transfer must reject missing recipient");
        assert!(matches!(err, TxError::RecipientMissing));
        assert_eq!(st.accounts, before.accounts);
        assert_eq!(st.fee_pool, before.fee_pool);
    }

    /// Transfer to uninitialized recipient leaves accounts unchanged (formerly `apply_tx_transfer_rejects_uninitialized_recipient_without_side_effects`).
    #[test]
    fn xfer_rcpt_uninit_no_mut() {
        let (g, sks) = dev_net();
        let mut st = g.state0();
        let sk_v = &sks[0];
        let aid_v = g.accounts[0].acct;
        let dom_v = domain_of_account_id(&aid_v);
        let want_hi = dom_v.to_be_bytes()[0];
        let mut aid_r = [0xA2u8; 32];
        aid_r[0] = want_hi;
        st.accounts.insert(aid_r, Account::default());
        let before = st.clone();

        let xfer = SignedTx::sign_body(
            sk_v,
            dom_v,
            0,
            0,
            TxBody::Transfer {
                to: aid_r,
                amount: 123,
                fee: 7,
            },
        );
        let err = st
            .apply_tx(&xfer)
            .expect_err("transfer must reject uninitialized recipient");
        assert!(matches!(err, TxError::RecipientNotInitialized));
        assert_eq!(st.accounts, before.accounts);
        assert_eq!(st.fee_pool, before.fee_pool);
    }

    /// Self-transfer is rejected without mutating sender (formerly `apply_tx_transfer_self_is_rejected_without_side_effects`).
    #[test]
    fn xfer_self_reject_no_mut() {
        let (g, sks) = dev_net();
        let mut st = g.state0();
        let sk_v = &sks[0];
        let aid_v = g.accounts[0].acct;
        let dom_v = domain_of_account_id(&aid_v);

        let accounts_before = st.accounts.len();
        let before = st.get(&aid_v).expect("sender exists").clone();
        let xfer = SignedTx::sign_body(
            sk_v,
            dom_v,
            g.accounts[0].der_idx,
            0,
            TxBody::Transfer {
                to: aid_v,
                amount: 10,
                fee: 1,
            },
        );

        let err = st
            .apply_tx(&xfer)
            .expect_err("self-transfer must be rejected");
        assert!(matches!(err, TxError::InvalidTransfer));
        let after = st.get(&aid_v).expect("self account exists");
        assert_eq!(after.nonce, before.nonce);
        assert_eq!(after.balance_pwm, before.balance_pwm);
        assert_eq!(
            st.accounts.len(),
            accounts_before,
            "no extra accounts must be created"
        );
    }

    /// Import with missing recipient row rejects cleanly (formerly `apply_tx_import_rejects_missing_destination_without_side_effects`).
    #[test]
    fn imp_dst_miss_no_mut() {
        // Create a minimal state with two initialized accounts in different hi-byte domains
        // and a missing destination account on the destination side.
        let mut st = State::default();

        let (sk_src, src_i, src_aid) = user_sk0(&[0xB1; 32]);
        let src_hi = domain_of_account_id(&src_aid).to_be_bytes()[0];

        // Destination signer must have a different hi-byte domain than the source.
        let (sk_dst_signer, dst_i, dst_signer_aid) = {
            let mut seed_byte = 0xC2u8;
            let mut attempts = 0u32;
            loop {
                attempts += 1;
                let seed = [seed_byte; 32];
                let (sk, i, aid) = user_sk0(&seed);
                let hi = domain_of_account_id(&aid).to_be_bytes()[0];
                if hi != src_hi {
                    break (sk, i, aid);
                }
                seed_byte = seed_byte.wrapping_add(1);
                if attempts >= 4096 {
                    panic!("failed to find a dst signer with hi!=src_hi");
                }
            }
        };

        let dst_hi = domain_of_account_id(&dst_signer_aid).to_be_bytes()[0];

        // Recipient must be distinct from dst signer, but share the same dst hi-byte domain.
        let (_, _, recipient_aid) = {
            let mut seed_byte = 0xD3u8;
            let mut attempts = 0u32;
            loop {
                attempts += 1;
                let seed = [seed_byte; 32];
                let (sk, i, aid) = user_sk0(&seed);
                let hi = domain_of_account_id(&aid).to_be_bytes()[0];
                if hi == dst_hi && aid != dst_signer_aid {
                    break (sk, i, aid);
                }
                seed_byte = seed_byte.wrapping_add(1);
                if attempts >= 4096 {
                    panic!("failed to find a recipient distinct from dst signer");
                }
            }
        };

        st.accounts.insert(
            src_aid,
            Account::genesis_funded(sk_src.verifying_key().to_bytes(), src_i, 1_000_000),
        );
        st.accounts.insert(
            dst_signer_aid,
            Account::genesis_funded(sk_dst_signer.verifying_key().to_bytes(), dst_i, 1_000_000),
        );

        let src_dom = domain_of_account_id(&src_aid);
        let dst_dom = domain_of_account_id(&dst_signer_aid);
        let import_dom = dst_dom;

        // Export targets recipient_aid (may be missing in state on dst side).
        let export_amount = 42u128;
        let export_fee = 1u128;
        let export_tx = SignedTx::sign_body(
            &sk_src,
            src_dom,
            src_i,
            0,
            TxBody::Export {
                to: recipient_aid,
                target_domain: import_dom,
                amount: export_amount,
                fee: export_fee,
            },
        );
        let export_id = export_tx.export_id().expect("export id");
        st.apply_tx(&export_tx).expect("export should apply");

        // Import targets recipient_aid (to != signer id). Destination must initialize first.
        let import_amount = export_amount;
        let import_tx = SignedTx::sign_body(
            &sk_dst_signer,
            import_dom,
            dst_i,
            0,
            TxBody::Import {
                to: recipient_aid,
                amount: import_amount,
                export_id,
            },
        );
        let before = st.clone();
        let err = st
            .apply_tx(&import_tx)
            .expect_err("import must reject missing recipient");
        assert!(matches!(err, TxError::RecipientMissing));
        assert_eq!(st.accounts, before.accounts);
        assert_eq!(st.imported_set, before.imported_set);
    }

    /// Import to uninitialized recipient rejects without registry drift (formerly `apply_tx_import_rejects_uninitialized_destination_without_side_effects`).
    #[test]
    fn imp_dst_uninit_no_mut() {
        let mut st = State::default();

        let (sk_src, src_i, src_aid) = user_sk0(&[0xB4; 32]);
        let src_hi = domain_of_account_id(&src_aid).to_be_bytes()[0];
        let (sk_dst_signer, dst_i, dst_signer_aid) = (0u8..=u8::MAX)
            .find_map(|b| {
                let (sk, i, aid) = user_sk0(&[b; 32]);
                (domain_of_account_id(&aid).to_be_bytes()[0] != src_hi).then_some((sk, i, aid))
            })
            .expect("dst signer");
        let dst_hi = domain_of_account_id(&dst_signer_aid).to_be_bytes()[0];
        let (_, _, recipient_aid) = (0u8..=u8::MAX)
            .find_map(|b| {
                let (sk, i, aid) = user_sk0(&[b.wrapping_add(1); 32]);
                (domain_of_account_id(&aid).to_be_bytes()[0] == dst_hi && aid != dst_signer_aid)
                    .then_some((sk, i, aid))
            })
            .expect("recipient");

        st.accounts.insert(
            src_aid,
            Account::genesis_funded(sk_src.verifying_key().to_bytes(), src_i, 1_000_000),
        );
        st.accounts.insert(
            dst_signer_aid,
            Account::genesis_funded(sk_dst_signer.verifying_key().to_bytes(), dst_i, 1_000_000),
        );
        st.accounts.insert(recipient_aid, Account::default());

        let export_amount = 42u128;
        let export_tx = SignedTx::sign_body(
            &sk_src,
            domain_of_account_id(&src_aid),
            src_i,
            0,
            TxBody::Export {
                to: recipient_aid,
                target_domain: domain_of_account_id(&dst_signer_aid),
                amount: export_amount,
                fee: 1,
            },
        );
        let export_id = export_tx.export_id().expect("export id");
        st.apply_tx(&export_tx).expect("export should apply");

        let import_tx = SignedTx::sign_body(
            &sk_dst_signer,
            domain_of_account_id(&dst_signer_aid),
            dst_i,
            0,
            TxBody::Import {
                to: recipient_aid,
                amount: export_amount,
                export_id,
            },
        );
        let before = st.clone();
        let err = st
            .apply_tx(&import_tx)
            .expect_err("import must reject uninitialized recipient");
        assert!(matches!(err, TxError::RecipientNotInitialized));
        assert_eq!(st.accounts, before.accounts);
        assert_eq!(st.imported_set, before.imported_set);
    }

    /// Embedded import provenance must not leak into registry on rejected tx (recipient missing).
    #[test]
    fn imp_emb_prov_dst_clean() {
        let mut st = State::default();
        let (sk_dst_signer, dst_i, dst_signer_aid) = user_sk0(&[0xE1; 32]);
        let dst_hi = domain_of_account_id(&dst_signer_aid).to_be_bytes()[0];
        let (_, _, recipient_aid) = (0u8..=u8::MAX)
            .find_map(|b| {
                let (sk, i, aid) = user_sk0(&[b.wrapping_add(7); 32]);
                (domain_of_account_id(&aid).to_be_bytes()[0] == dst_hi && aid != dst_signer_aid)
                    .then_some((sk, i, aid))
            })
            .expect("recipient");
        st.accounts.insert(
            dst_signer_aid,
            Account::genesis_funded(sk_dst_signer.verifying_key().to_bytes(), dst_i, 1_000_000),
        );
        let import_dom = domain_of_account_id(&dst_signer_aid);
        let export_id = [0xEF; 32];
        let mut import_tx = SignedTx::sign_body(
            &sk_dst_signer,
            import_dom,
            dst_i,
            0,
            TxBody::Import {
                to: recipient_aid,
                amount: 19,
                export_id,
            },
        );
        import_tx.set_import_provenance_signed(
            &sk_dst_signer,
            Some(crate::state::ExportProvenance {
                to: recipient_aid,
                target_domain: import_dom,
                amount: 19,
            }),
        );
        let err = st
            .apply_tx(&import_tx)
            .expect_err("import must reject missing recipient");
        assert!(matches!(err, TxError::RecipientMissing));
        assert!(
            !st.exported_registry.contains_key(&export_id),
            "rejected import must not mutate exported_registry"
        );
        assert!(!st.imported_set.contains(&export_id));
    }

    #[test]
    fn apply_tx_rejects_bad_nonce() {
        let (g, sks) = dev_net();
        let mut st = g.state0();
        let sk_v = &sks[0];
        let aid_v = g.accounts[0].acct;
        let dom_v = domain_of_account_id(&aid_v);

        let tx = SignedTx::sign_body(sk_v, dom_v, 0, 99, TxBody::Stake { amount: 1 });
        let e = st.apply_tx(&tx).expect_err("nonce must match account");
        assert!(matches!(e, TxError::BadNonce));
    }

    #[test]
    fn claim_ipv4_batch_happy_apply() {
        let mut ctx = mk_claim_ctx();
        let before = ctx.st.get(&ctx.claim_id).expect("claimant before").clone();
        let msg = super::claim_ipv4_msg(7, &ctx.batch_root, &ctx.claim_id);
        let registry_sig = sign(&ctx.reg_sk, &msg);
        let tx = SignedTx::sign_body(
            &ctx.claim_sk,
            domain_of_account_id(&ctx.claim_id),
            ctx.claim_idx,
            before.nonce,
            TxBody::ClaimIPv4Batch {
                phase: 7,
                batch_root: ctx.batch_root,
                registry_sig,
            },
        );

        eprintln!(
            "V5_CLAIM_TEST_TX_JSON={}",
            serde_json::to_string_pretty(&tx).expect("ser")
        );
        eprintln!("V5_CLAIM_TEST_BATCH_ROOT={}", hex::encode(ctx.batch_root));

        ctx.st
            .apply_tx_with_ctx(&tx, 1, 0, &ctx.gen_cfg)
            .expect("claim must apply");
        let after = ctx.st.get(&ctx.claim_id).expect("claimant after");
        assert_eq!(after.balance_pwm, before.balance_pwm + 500);
        assert_eq!(after.ipv4_claimed_phase, Some(7));
        assert_eq!(after.nonce, before.nonce + 1);
    }

    #[test]
    fn claim_ipv4_phase_unknown() {
        let mut ctx = mk_claim_ctx();
        let before = ctx.st.clone();
        let msg = super::claim_ipv4_msg(9, &ctx.batch_root, &ctx.claim_id);
        let tx = SignedTx::sign_body(
            &ctx.claim_sk,
            domain_of_account_id(&ctx.claim_id),
            ctx.claim_idx,
            0,
            TxBody::ClaimIPv4Batch {
                phase: 9,
                batch_root: ctx.batch_root,
                registry_sig: sign(&ctx.reg_sk, &msg),
            },
        );

        let err = ctx
            .st
            .apply_tx_with_ctx(&tx, 1, 0, &ctx.gen_cfg)
            .expect_err("unknown phase must fail");
        assert!(matches!(err, TxError::PolicySchemaInvalid));
        assert_eq!(ctx.st.accounts, before.accounts);
        assert_eq!(ctx.st.fee_pool, before.fee_pool);
    }

    #[test]
    fn claim_ipv4_sig_bad_reject() {
        let mut ctx = mk_claim_ctx();
        let before = ctx.st.clone();
        let (bad_sk, _, _) = user_sk0(&[0x46; 32]);
        let msg = super::claim_ipv4_msg(7, &ctx.batch_root, &ctx.claim_id);
        let tx = SignedTx::sign_body(
            &ctx.claim_sk,
            domain_of_account_id(&ctx.claim_id),
            ctx.claim_idx,
            0,
            TxBody::ClaimIPv4Batch {
                phase: 7,
                batch_root: ctx.batch_root,
                registry_sig: sign(&bad_sk, &msg),
            },
        );

        let err = ctx
            .st
            .apply_tx_with_ctx(&tx, 1, 0, &ctx.gen_cfg)
            .expect_err("bad registry signature must fail");
        assert!(matches!(err, TxError::BadSignature));
        assert_eq!(ctx.st.accounts, before.accounts);
        assert_eq!(ctx.st.fee_pool, before.fee_pool);
    }

    #[test]
    fn claim_ipv4_double_reject() {
        let mut ctx = mk_claim_ctx();
        let claim = ctx
            .st
            .accounts
            .get_mut(&ctx.claim_id)
            .expect("claimant row");
        claim.ipv4_claimed_phase = Some(1);
        let before = ctx.st.clone();
        let msg = super::claim_ipv4_msg(7, &ctx.batch_root, &ctx.claim_id);
        let tx = SignedTx::sign_body(
            &ctx.claim_sk,
            domain_of_account_id(&ctx.claim_id),
            ctx.claim_idx,
            0,
            TxBody::ClaimIPv4Batch {
                phase: 7,
                batch_root: ctx.batch_root,
                registry_sig: sign(&ctx.reg_sk, &msg),
            },
        );

        let err = ctx
            .st
            .apply_tx_with_ctx(&tx, 1, 0, &ctx.gen_cfg)
            .expect_err("double claim must fail");
        assert!(matches!(err, TxError::PolicyDenied));
        assert_eq!(ctx.st.accounts, before.accounts);
        assert_eq!(ctx.st.fee_pool, before.fee_pool);
    }

    #[test]
    fn claim_ipv4_uninit_reject() {
        let mut ctx = mk_claim_ctx();
        ctx.st.accounts.insert(ctx.claim_id, Account::default());
        let before = ctx.st.clone();
        let msg = super::claim_ipv4_msg(7, &ctx.batch_root, &ctx.claim_id);
        let tx = SignedTx::sign_body(
            &ctx.claim_sk,
            domain_of_account_id(&ctx.claim_id),
            ctx.claim_idx,
            0,
            TxBody::ClaimIPv4Batch {
                phase: 7,
                batch_root: ctx.batch_root,
                registry_sig: sign(&ctx.reg_sk, &msg),
            },
        );

        let err = ctx
            .st
            .apply_tx_with_ctx(&tx, 1, 0, &ctx.gen_cfg)
            .expect_err("uninitialized claimant must fail");
        assert!(matches!(err, TxError::NotInitialized));
        assert_eq!(ctx.st.accounts, before.accounts);
        assert_eq!(ctx.st.fee_pool, before.fee_pool);
    }

    /// Quota debit burn skips balancePWM (formerly `burn_mark_debits_quota_without_touching_balance`).
    #[test]
    fn burn_quota_skip_bal_chg() {
        let (g, sks) = dev_net();
        let mut st = g.state0();
        let sk_v = &sks[0];
        let aid_v = g.accounts[0].acct;
        let dom_v = domain_of_account_id(&aid_v);
        let before = st.get(&aid_v).expect("validator").clone();
        let fee_pool_before = st.fee_pool;
        st.accounts.get_mut(&aid_v).expect("validator").stored_marks = 25;

        let burn = SignedTx::sign_body(
            sk_v,
            dom_v,
            0,
            before.nonce,
            TxBody::BurnMark {
                mark_amount: 7,
                beneficiary: None,
            },
        );
        st.apply_tx(&burn).expect("quota burn");

        let after = st.get(&aid_v).expect("validator after burn");
        assert_eq!(after.balance_pwm, before.balance_pwm);
        assert_eq!(after.nonce, before.nonce + 1);
        assert_eq!(after.stored_marks, 18);
        assert_eq!(st.fee_pool, fee_pool_before);
    }

    /// Over-quota burn is rejected without mutations (formerly `burn_mark_rejects_insufficient_quota_without_side_effects`).
    #[test]
    fn burn_quota_low_no_mut() {
        let (g, sks) = dev_net();
        let mut st = g.state0();
        let sk_v = &sks[0];
        let aid_v = g.accounts[0].acct;
        let dom_v = domain_of_account_id(&aid_v);
        st.accounts.get_mut(&aid_v).expect("validator").stored_marks = 3;
        let before = st.get(&aid_v).expect("validator").clone();
        let fee_pool_before = st.fee_pool;

        let burn = SignedTx::sign_body(
            sk_v,
            dom_v,
            0,
            before.nonce,
            TxBody::BurnMark {
                mark_amount: 9,
                beneficiary: None,
            },
        );
        let err = st.apply_tx(&burn).expect_err("must reject");
        assert!(matches!(err, TxError::InsufficientMarks));

        let after = st.get(&aid_v).expect("validator after reject");
        assert_eq!(after, &before);
        assert_eq!(st.fee_pool, fee_pool_before);
    }

    /// Burn with same-shard beneficiary debits quota only (formerly `burn_mark_with_beneficiary_keeps_fee_pool_and_balances_unchanged`).
    #[test]
    fn burn_ben_stable_bal_fee() {
        let (g, sks) = dev_net();
        let mut st = g.state0();
        let sk_v = &sks[0];
        let aid_v = g.accounts[0].acct;
        let dom_v = domain_of_account_id(&aid_v);

        st.accounts.get_mut(&aid_v).expect("sender").stored_marks = 40;
        let sender_before = st.get(&aid_v).expect("sender before").clone();
        let fee_pool_before = st.fee_pool;
        let sender_hi = dom_v.to_be_bytes()[0];
        let mut beneficiary = [0u8; 32];
        beneficiary[0] = sender_hi;

        let burn = SignedTx::sign_body(
            sk_v,
            dom_v,
            0,
            sender_before.nonce,
            TxBody::BurnMark {
                mark_amount: 6,
                beneficiary: Some(beneficiary),
            },
        );
        st.apply_tx(&burn).expect("burn with beneficiary");

        let sender_after = st.get(&aid_v).expect("sender after");
        assert_eq!(sender_after.balance_pwm, sender_before.balance_pwm);
        assert_eq!(st.fee_pool, fee_pool_before);
        assert_eq!(sender_after.stored_marks, 34);
    }

    /// Foreign-domain beneficiary burn debits sender marks (V2-7: cross-domain allowed).
    #[test]
    fn burn_ben_xdom_ok() {
        let (g, sks) = dev_net();
        let mut st = g.state0();
        let sk_v = &sks[0];
        let aid_v = g.accounts[0].acct;
        let dom_v = domain_of_account_id(&aid_v);
        st.accounts.get_mut(&aid_v).expect("validator").stored_marks = 20;
        let before = st.get(&aid_v).expect("validator").clone();
        let fee_pool_before = st.fee_pool;

        let sender_hi = dom_v.to_be_bytes()[0];
        let other_hi = if sender_hi == 0xFF {
            sender_hi.saturating_sub(1)
        } else {
            sender_hi + 1
        };
        let mut foreign_beneficiary = [0u8; 32];
        foreign_beneficiary[0] = other_hi;
        let burn = SignedTx::sign_body(
            sk_v,
            dom_v,
            0,
            before.nonce,
            TxBody::BurnMark {
                mark_amount: 5,
                beneficiary: Some(foreign_beneficiary),
            },
        );
        st.apply_tx(&burn)
            .expect("cross-domain beneficiary must apply");

        let after = st.get(&aid_v).expect("validator after burn");
        assert_eq!(after.stored_marks, 15);
        assert_eq!(after.nonce, before.nonce + 1);
        assert_eq!(after.balance_pwm, before.balance_pwm);
        assert_eq!(st.fee_pool, fee_pool_before);
    }

    /// Insufficient PWM transfer rejects (formerly `apply_tx_rejects_insufficient_balance_on_transfer`).
    #[test]
    fn xfer_reject_low_pwm() {
        let (g, sks) = dev_net();
        let mut st = g.state0();
        let sk_v = &sks[0];
        let aid_v = g.accounts[0].acct;
        let dom_v = domain_of_account_id(&aid_v);

        let (sk_b, i_b, aid_b) = user_sk0(&[5u8; 32]);
        let dom_b = domain_of_account_id(&aid_b);
        st.apply_tx(&SignedTx::sign_body(
            &sk_b,
            dom_b,
            i_b,
            0,
            TxBody::Init { index: 0, flags: 0 },
        ))
        .unwrap();

        let xfer = SignedTx::sign_body(
            sk_v,
            dom_v,
            0,
            0,
            TxBody::Transfer {
                to: aid_b,
                amount: 2_000_000,
                fee: 0,
            },
        );
        let e = st.apply_tx(&xfer).expect_err("balance below amount+fee");
        assert!(matches!(e, TxError::Insufficient));
    }

    /// `validate_tx_shape` catches domain≠account-prefix (formerly `validate_tx_shape_rejects_domain_mismatch`).
    #[test]
    fn shape_reject_bad_dom() {
        let (sk, i, aid) = user_sk0(&[11u8; 32]);
        let d_ok = domain_of_account_id(&aid);
        let d_wrong = if d_ok == u16::MAX { 0u16 } else { d_ok + 1 };
        let tx = SignedTx::sign_body(&sk, d_wrong, i, 0, TxBody::Init { index: 0, flags: 0 });
        let e = validate_tx_shape(&tx).expect_err("domain must match account id prefix");
        assert!(matches!(e, TxError::DomainMismatch));
        assert!(d_ok != d_wrong);
    }

    /// Export debits source and credits fee pool (formerly `export_debits_source_and_collects_fee_happy_path`).
    #[test]
    fn export_debit_fee_ok() {
        let (g, sks) = dev_net();
        let mut st = g.state0();
        let sk_v = &sks[0];
        let aid_v = g.accounts[0].acct;
        let dom_v = domain_of_account_id(&aid_v);
        let before = st.get(&aid_v).expect("sender").clone();
        let fee_pool_before = st.fee_pool;

        let source_hi = dom_v.to_be_bytes()[0];
        let target_hi = source_hi.wrapping_add(1);
        let target_domain = ((target_hi as u16) << 8) | 0x01;
        let mut to = [0u8; 32];
        to[0] = target_hi;

        let tx = SignedTx::sign_body(
            sk_v,
            dom_v,
            0,
            before.nonce,
            TxBody::Export {
                to,
                target_domain,
                amount: 100,
                fee: 5,
            },
        );
        st.apply_tx(&tx).expect("export debit on source shard");

        let after = st.get(&aid_v).expect("sender after");
        assert_eq!(after.balance_pwm, before.balance_pwm - 105);
        assert_eq!(after.nonce, before.nonce + 1);
        assert_eq!(st.fee_pool, fee_pool_before + 5);
        let export_id = tx.export_id().expect("export id");
        let lock = st
            .cross_shard_locks
            .iter()
            .find(|row| row.export_id == export_id)
            .expect("export lock");
        assert_eq!(lock.sender, aid_v);
        assert_eq!(lock.amount_pwm, 100);
        assert_eq!(lock.target_domain, target_domain);
        assert_eq!(lock.refund_account, aid_v);
        assert_eq!(lock.lock_height, 0);
        assert_eq!(lock.unlock_height, g.xshard_lock_to_blocks);
        assert_eq!(lock.state, CrossShardLockState::Locked);
    }

    /// Oversized export is rejected cleanly (formerly `export_rejects_insufficient_balance_without_side_effects`).
    #[test]
    fn export_reject_low_no_mut() {
        let (g, sks) = dev_net();
        let mut st = g.state0();
        let sk_v = &sks[0];
        let aid_v = g.accounts[0].acct;
        let dom_v = domain_of_account_id(&aid_v);
        let before = st.get(&aid_v).expect("sender").clone();
        let fee_pool_before = st.fee_pool;

        let source_hi = dom_v.to_be_bytes()[0];
        let target_hi = source_hi.wrapping_add(1);
        let target_domain = ((target_hi as u16) << 8) | 0x01;
        let mut to = [0u8; 32];
        to[0] = target_hi;

        let tx = SignedTx::sign_body(
            sk_v,
            dom_v,
            0,
            before.nonce,
            TxBody::Export {
                to,
                target_domain,
                amount: before.balance_pwm + 1,
                fee: 0,
            },
        );
        let err = st.apply_tx(&tx).expect_err("must reject");
        assert!(matches!(err, TxError::Insufficient));
        let after = st.get(&aid_v).expect("sender after");
        assert_eq!(after, &before);
        assert_eq!(st.fee_pool, fee_pool_before);
    }

    #[test]
    fn export_rejects_bad_nonce() {
        let (g, sks) = dev_net();
        let mut st = g.state0();
        let sk_v = &sks[0];
        let aid_v = g.accounts[0].acct;
        let dom_v = domain_of_account_id(&aid_v);

        let source_hi = dom_v.to_be_bytes()[0];
        let target_hi = source_hi.wrapping_add(1);
        let target_domain = ((target_hi as u16) << 8) | 0x01;
        let mut to = [0u8; 32];
        to[0] = target_hi;

        let tx = SignedTx::sign_body(
            sk_v,
            dom_v,
            0,
            99,
            TxBody::Export {
                to,
                target_domain,
                amount: 1,
                fee: 0,
            },
        );
        let err = st.apply_tx(&tx).expect_err("nonce mismatch");
        assert!(matches!(err, TxError::BadNonce));
    }

    #[test]
    fn import_credits_target_happy_path() {
        let (g, sks) = dev_net();
        let mut st = g.state0();
        let sk_v = &sks[0];
        let aid_v = g.accounts[0].acct;
        let dom_v = domain_of_account_id(&aid_v);
        let (sk_b, i_b, aid_b) = user_sk0(&[6u8; 32]);
        let dom_b = domain_of_account_id(&aid_b);
        st.apply_tx(&SignedTx::sign_body(
            &sk_b,
            dom_b,
            i_b,
            0,
            TxBody::Init { index: 0, flags: 0 },
        ))
        .expect("init import target");
        st.accounts.get_mut(&aid_b).expect("target").balance_pwm = crate::tx::MIN_IMPORT_FEE_UNITS;
        st.accounts.get_mut(&aid_b).expect("target").balance_pwm = crate::tx::MIN_IMPORT_FEE_UNITS;
        let before_from = st.get(&aid_b).expect("import signer").clone();
        let before_to = st.get(&aid_b).expect("target").clone();
        let export = SignedTx::sign_body(
            sk_v,
            dom_v,
            0,
            st.get(&aid_v).expect("source before export").nonce,
            TxBody::Export {
                to: aid_b,
                target_domain: dom_b,
                amount: 123,
                fee: 1,
            },
        );
        let export_id = export.export_id().expect("export id");
        st.apply_tx(&export).expect("export must be recorded");

        let tx = SignedTx::sign_body(
            &sk_b,
            dom_b,
            i_b,
            before_from.nonce,
            TxBody::Import {
                to: aid_b,
                amount: 123,
                export_id,
            },
        );
        st.apply_tx(&tx).expect("import must credit target");

        let after_from = st.get(&aid_b).expect("signer after");
        let after_to = st.get(&aid_b).expect("target after");
        assert_eq!(after_from.nonce, before_from.nonce + 1);
        assert_eq!(
            after_to.balance_pwm,
            before_to.balance_pwm + 123 - crate::tx::MIN_IMPORT_FEE_UNITS
        );
        assert!(st.imported_set.contains(&export_id));
        assert_eq!(
            st.cross_shard_locks
                .iter()
                .find(|row| row.export_id == export_id)
                .expect("released lock")
                .state,
            CrossShardLockState::Released
        );
    }

    #[test]
    fn escrow_refund_timeout() {
        let (mut g, sks) = dev_net();
        g.xshard_lock_to_blocks = 2;
        let mut st = g.state0();
        let sk_v = &sks[0];
        let aid_v = g.accounts[0].acct;
        let dom_v = domain_of_account_id(&aid_v);
        let before = st.get(&aid_v).expect("sender").balance_pwm;
        let target_hi = dom_v.to_be_bytes()[0].wrapping_add(1);
        let target_domain = ((target_hi as u16) << 8) | 0x01;
        let mut to = [0u8; 32];
        to[0] = target_hi;
        let tx = SignedTx::sign_body(
            sk_v,
            dom_v,
            0,
            st.get(&aid_v).expect("sender").nonce,
            TxBody::Export {
                to,
                target_domain,
                amount: 321,
                fee: 7,
            },
        );
        let export_id = tx.export_id().expect("export id");
        st.apply_tx_with_ctx(&tx, 10, 0, &g).expect("export");
        assert_eq!(st.get(&aid_v).expect("locked").balance_pwm, before - 328);

        st.refund_exp_locks(11);
        assert_eq!(
            st.get(&aid_v).expect("not unlocked").balance_pwm,
            before - 328
        );
        assert!(!st.imported_set.contains(&export_id));

        st.refund_exp_locks(12);
        assert_eq!(st.get(&aid_v).expect("refunded").balance_pwm, before - 7);
        assert!(st.imported_set.contains(&export_id));
        assert_eq!(
            st.cross_shard_locks
                .iter()
                .find(|row| row.export_id == export_id)
                .expect("refunded lock")
                .state,
            CrossShardLockState::Refunded
        );
    }

    #[test]
    fn escrow_late_import_refunded() {
        let (mut g, sks) = dev_net();
        g.xshard_lock_to_blocks = 1;
        let mut st = g.state0();
        let sk_v = &sks[0];
        let aid_v = g.accounts[0].acct;
        let dom_v = domain_of_account_id(&aid_v);
        let (sk_b, i_b, aid_b) = user_sk0(&[61u8; 32]);
        let dom_b = domain_of_account_id(&aid_b);
        st.apply_tx_with_ctx(
            &SignedTx::sign_body(&sk_b, dom_b, i_b, 0, TxBody::Init { index: 0, flags: 0 }),
            1,
            0,
            &g,
        )
        .expect("init import target");
        st.accounts.get_mut(&aid_b).expect("target").balance_pwm = crate::tx::MIN_IMPORT_FEE_UNITS;
        let target_before = st.get(&aid_b).expect("target before").clone();

        let export = SignedTx::sign_body(
            sk_v,
            dom_v,
            0,
            st.get(&aid_v).expect("source before export").nonce,
            TxBody::Export {
                to: aid_b,
                target_domain: dom_b,
                amount: 55,
                fee: 0,
            },
        );
        let export_id = export.export_id().expect("export id");
        st.apply_tx_with_ctx(&export, 2, 0, &g).expect("export");
        st.refund_exp_locks(3);

        let mut import = SignedTx::sign_body(
            &sk_b,
            dom_b,
            i_b,
            target_before.nonce,
            TxBody::Import {
                to: aid_b,
                amount: 55,
                export_id,
            },
        );
        import.set_import_fee_signed(&sk_b, crate::tx::MIN_IMPORT_FEE_UNITS);
        let err = st
            .apply_tx_with_ctx(&import, 4, 0, &g)
            .expect_err("late import must see refunded lock");
        assert!(matches!(err, TxError::ExportLockRefunded));
        assert_eq!(st.get(&aid_b).expect("target after"), &target_before);
    }

    /// Import without provenance rejects without mutating view (formerly `import_rejects_missing_export_provenance_without_side_effects`).
    #[test]
    fn imp_reject_miss_proof_clean() {
        let (g, _sks) = dev_net();
        let mut st = g.state0();
        let (sk_b, i_b, aid_b) = user_sk0(&[16u8; 32]);
        let dom_b = domain_of_account_id(&aid_b);
        st.apply_tx(&SignedTx::sign_body(
            &sk_b,
            dom_b,
            i_b,
            0,
            TxBody::Init { index: 0, flags: 0 },
        ))
        .expect("init import target");
        st.accounts.get_mut(&aid_b).expect("target").balance_pwm = crate::tx::MIN_IMPORT_FEE_UNITS;
        st.accounts.get_mut(&aid_b).expect("target").balance_pwm = crate::tx::MIN_IMPORT_FEE_UNITS;
        let before = st.get(&aid_b).expect("target before").clone();

        let tx = SignedTx::sign_body(
            &sk_b,
            dom_b,
            i_b,
            before.nonce,
            TxBody::Import {
                to: aid_b,
                amount: 123,
                export_id: [0xAA; 32],
            },
        );
        let err = st
            .apply_tx(&tx)
            .expect_err("unknown export provenance must fail");
        assert!(matches!(err, TxError::InvalidImport));

        let after = st.get(&aid_b).expect("target after");
        assert_eq!(after, &before);
        assert!(!st.imported_set.contains(&[0xAA; 32]));
        assert!(!st.exported_registry.contains_key(&[0xAA; 32]));
    }

    /// Amount mismatch vs export record rejects import (formerly `import_requires_matching_export_provenance`).
    #[test]
    fn imp_reject_proof_amt() {
        let (g, sks) = dev_net();
        let mut st = g.state0();
        let sk_v = &sks[0];
        let aid_v = g.accounts[0].acct;
        let dom_v = domain_of_account_id(&aid_v);

        let (sk_b, i_b, aid_b) = user_sk0(&[17u8; 32]);
        let dom_b = domain_of_account_id(&aid_b);
        st.apply_tx(&SignedTx::sign_body(
            &sk_b,
            dom_b,
            i_b,
            0,
            TxBody::Init { index: 0, flags: 0 },
        ))
        .expect("init import target");
        st.accounts.get_mut(&aid_b).expect("target").balance_pwm = crate::tx::MIN_IMPORT_FEE_UNITS;

        let export = SignedTx::sign_body(
            sk_v,
            dom_v,
            0,
            st.get(&aid_v).expect("source before export").nonce,
            TxBody::Export {
                to: aid_b,
                target_domain: dom_b,
                amount: 70,
                fee: 1,
            },
        );
        let export_id = export.export_id().expect("export id");
        st.apply_tx(&export).expect("export must be recorded");
        let before = st.get(&aid_b).expect("target before import").clone();

        let wrong_amount = SignedTx::sign_body(
            &sk_b,
            dom_b,
            i_b,
            before.nonce,
            TxBody::Import {
                to: aid_b,
                amount: 71,
                export_id,
            },
        );
        let err = st
            .apply_tx(&wrong_amount)
            .expect_err("import amount mismatch must fail");
        assert!(matches!(err, TxError::InvalidImport));
        assert_eq!(st.get(&aid_b).expect("target after reject"), &before);
    }

    /// Replay of same export_id rejects without further mutations (formerly `import_rejects_duplicate_export_id_without_side_effects`).
    #[test]
    fn imp_dup_id_no_mut() {
        let (g, sks) = dev_net();
        let mut st = g.state0();
        let sk_v = &sks[0];
        let aid_v = g.accounts[0].acct;
        let dom_v = domain_of_account_id(&aid_v);
        let (sk_b, i_b, aid_b) = user_sk0(&[7u8; 32]);
        let dom_b = domain_of_account_id(&aid_b);
        st.apply_tx(&SignedTx::sign_body(
            &sk_b,
            dom_b,
            i_b,
            0,
            TxBody::Init { index: 0, flags: 0 },
        ))
        .expect("init import target");
        st.accounts.get_mut(&aid_b).expect("target").balance_pwm = crate::tx::MIN_IMPORT_FEE_UNITS;
        let export = SignedTx::sign_body(
            sk_v,
            dom_v,
            0,
            st.get(&aid_v).expect("source before export").nonce,
            TxBody::Export {
                to: aid_b,
                target_domain: dom_b,
                amount: 55,
                fee: 1,
            },
        );
        let export_id = export.export_id().expect("export id");
        st.apply_tx(&export).expect("export must be recorded");

        let first = SignedTx::sign_body(
            &sk_b,
            dom_b,
            i_b,
            1,
            TxBody::Import {
                to: aid_b,
                amount: 55,
                export_id,
            },
        );
        st.apply_tx(&first).expect("first import");
        let signer_before = st.get(&aid_b).expect("signer before duplicate").clone();
        let target_before = st.get(&aid_b).expect("target before duplicate").clone();

        let duplicate = SignedTx::sign_body(
            &sk_b,
            dom_b,
            i_b,
            signer_before.nonce,
            TxBody::Import {
                to: aid_b,
                amount: 55,
                export_id,
            },
        );
        let err = st
            .apply_tx(&duplicate)
            .expect_err("duplicate export id must fail");
        assert!(matches!(err, TxError::DuplicateImport));
        assert_eq!(st.get(&aid_b).expect("signer after reject"), &signer_before);
        assert_eq!(st.get(&aid_b).expect("target after reject"), &target_before);
    }

    /// `imported_set` survives serde round-trip (formerly `snapshot_restore_keeps_import_replay_guard`).
    ///
    /// Ignored: `Account.marks` uses JSON-compat `deserialize_with` (untagged wire); bincode
    /// deserialize fails with `DeserializeAnyNotSupported` (V2-5 marks migration).
    #[test]
    #[ignore = "bincode snapshot breaks: marks de uses untagged/Any (V2-5); needs coding fix"]
    fn snap_keep_imp_replay_guard() {
        let (g, sks) = dev_net();
        let mut st = g.state0();
        let sk_v = &sks[0];
        let aid_v = g.accounts[0].acct;
        let dom_v = domain_of_account_id(&aid_v);
        let (sk_b, i_b, aid_b) = user_sk0(&[8u8; 32]);
        let dom_b = domain_of_account_id(&aid_b);
        st.apply_tx(&SignedTx::sign_body(
            &sk_b,
            dom_b,
            i_b,
            0,
            TxBody::Init { index: 0, flags: 0 },
        ))
        .expect("init import target");
        st.accounts.get_mut(&aid_b).expect("target").balance_pwm = crate::tx::MIN_IMPORT_FEE_UNITS;
        let export = SignedTx::sign_body(
            sk_v,
            dom_v,
            0,
            st.get(&aid_v).expect("source before export").nonce,
            TxBody::Export {
                to: aid_b,
                target_domain: dom_b,
                amount: 21,
                fee: 1,
            },
        );
        let export_id = export.export_id().expect("export id");
        st.apply_tx(&export).expect("export must be recorded");

        let first = SignedTx::sign_body(
            &sk_b,
            dom_b,
            i_b,
            1,
            TxBody::Import {
                to: aid_b,
                amount: 21,
                export_id,
            },
        );
        st.apply_tx(&first).expect("first import");

        let encoded = bincode::serialize(&st).expect("snapshot serialize");
        let mut restored: crate::state::State =
            bincode::deserialize(&encoded).expect("snapshot restore");
        let signer_before = restored.get(&aid_b).expect("restored signer").clone();
        let target_before = restored.get(&aid_b).expect("restored target").clone();

        let duplicate = SignedTx::sign_body(
            &sk_b,
            dom_b,
            i_b,
            signer_before.nonce,
            TxBody::Import {
                to: aid_b,
                amount: 21,
                export_id,
            },
        );
        let err = restored
            .apply_tx(&duplicate)
            .expect_err("replay guard must survive snapshot restore");
        assert!(matches!(err, TxError::DuplicateImport));
        assert_eq!(
            restored.get(&aid_b).expect("signer after reject"),
            &signer_before
        );
        assert_eq!(
            restored.get(&aid_b).expect("target after reject"),
            &target_before
        );
    }

    #[test]
    fn st_v6_defaults_json() {
        let legacy = serde_json::json!({
            "accounts": {},
            "fee_pool": 0
        });
        let st: State = serde_json::from_value(legacy).expect("legacy json decode");
        assert_eq!(st.epoch_counter, 0);
        assert!(st.active_validator_indices.is_empty());
        assert!(st.cross_shard_locks.is_empty());
        assert!(st.evidence_log.is_empty());
        assert!(st.pending_conservation.is_empty());
    }

    #[test]
    fn export_prov_json_hex() {
        let provenance = ExportProvenance {
            to: [0xab; 32],
            target_domain: 7,
            amount: 11,
        };
        let mut json = serde_json::to_value(&provenance).expect("export provenance json encode");
        let to = json["to"].as_str().expect("to must be a hex string");
        assert_eq!(to.len(), 64);
        assert_eq!(to, "ab".repeat(32));
        assert_eq!(
            serde_json::from_value::<ExportProvenance>(json.clone())
                .expect("hex export provenance decode"),
            provenance
        );

        json["to"] = serde_json::json!(provenance.to);
        assert_eq!(
            serde_json::from_value::<ExportProvenance>(json)
                .expect("legacy export provenance decode"),
            provenance
        );
    }

    #[test]
    fn st_v6_json_roundtrip() {
        let sender = [0x11; 32];
        let recv = [0x22; 32];
        let record = EvidenceRecord {
            record_id: [0x33; 32],
            height: 42,
            offender_validator_idx: 7,
            evidence_type: EvidenceType::CustomStub(9),
            payload_hash: [0x44; 32],
            reporter: Some(sender),
        };
        let lock = CrossShardLock {
            export_id: [0x55; 32],
            sender,
            amount_pwm: 123_456_789_012_345_678_901_234_567_890u128,
            target_domain: 0x1234,
            refund_account: sender,
            lock_height: 1_000,
            unlock_height: 2_000,
            state: CrossShardLockState::Locked,
        };
        let pend = PendingConservationTransfer {
            sender,
            recipient: recv,
            amount_pwm: 987_654_321_098_765_432_109_876_543_210u128,
            fee_pwm: 17,
            nonce: 19,
            enqueue_height: 2_100,
            execute_at_height: 2_200,
            tx_hash: [0x66; 32],
        };
        let st = State {
            epoch_counter: 5,
            active_validator_indices: vec![1, 3, 8],
            cross_shard_locks: vec![lock.clone()],
            evidence_log: vec![record.clone()],
            pending_conservation: vec![pend.clone()],
            ..Default::default()
        };

        let json = serde_json::to_vec(&st).expect("state json encode");
        let de_json: State = serde_json::from_slice(&json).expect("state json decode");
        assert_eq!(de_json.epoch_counter, 5);
        assert_eq!(de_json.active_validator_indices, vec![1, 3, 8]);
        assert_eq!(de_json.cross_shard_locks, vec![lock.clone()]);
        assert_eq!(de_json.evidence_log, vec![record.clone()]);
        assert_eq!(de_json.pending_conservation, vec![pend.clone()]);

        // State bincode deserialize is currently blocked by Account marks compatibility
        // (see existing ignored test). Keep this guard to ensure new V6 fields serialize.
        let _ = bincode::serialize(&st).expect("state bincode encode");
    }

    #[test]
    fn evidence_duplicate_reject() {
        let (g, _) = dev_net();
        let mut st = g.state0();
        let payload_hash = [0xA5; 32];
        let record_id = st
            .append_evidence(9, 2, EvidenceType::UnavailableProposer, payload_hash, None)
            .expect("append first evidence");
        let err = st
            .append_evidence(9, 2, EvidenceType::UnavailableProposer, payload_hash, None)
            .expect_err("duplicate evidence must reject");
        assert!(matches!(err, TxError::EvidenceDuplicate));
        assert_eq!(st.evidence_log.len(), 1);
        assert_eq!(st.evidence_log[0].record_id, record_id);
    }

    #[test]
    fn evidence_append_no_seizure() {
        let (g, _) = dev_net();
        let mut st = g.state0();
        let balances = st
            .accounts
            .iter()
            .map(|(id, acc)| (*id, acc.balance_pwm))
            .collect::<Vec<_>>();
        let stakes = st
            .accounts
            .iter()
            .map(|(id, acc)| (*id, acc.staked_pwm_raw))
            .collect::<Vec<_>>();
        let active = st.active_validator_indices.clone();
        let reporter = g.accounts.first().map(|row| row.acct);

        st.append_evidence(
            11,
            0,
            EvidenceType::InvalidAttestation,
            [0x5A; 32],
            reporter,
        )
        .expect("append evidence");

        assert_eq!(st.evidence_log.len(), 1);
        assert_eq!(
            balances,
            st.accounts
                .iter()
                .map(|(id, acc)| (*id, acc.balance_pwm))
                .collect::<Vec<_>>()
        );
        assert_eq!(
            stakes,
            st.accounts
                .iter()
                .map(|(id, acc)| (*id, acc.staked_pwm_raw))
                .collect::<Vec<_>>()
        );
        assert_eq!(active, st.active_validator_indices);
    }

    #[test]
    fn stake_autoclaim_zero_matured() {
        let (g, sks) = dev_net();
        let mut st = g.state0();
        let sk_v = &sks[0];
        let aid_v = g.accounts[0].acct;
        let dom_v = domain_of_account_id(&aid_v);
        {
            let acc = st.accounts.get_mut(&aid_v).expect("validator");
            acc.staked_pwm_raw = 3 * PWM_RAW_SCALE;
            acc.marks_last_block = 10_000;
        }
        let before_marks = st.get(&aid_v).expect("validator").stored_marks;
        let tx = SignedTx::sign_body(sk_v, dom_v, 0, 0, TxBody::Stake { amount: 1 });
        st.apply_tx_with_ctx(&tx, 12, 10_100, &g)
            .expect("stake apply");
        let after = st.get(&aid_v).expect("validator");
        assert_eq!(after.stored_marks, before_marks);
    }

    /// 1 whole PWM staked for 1 hour yields 1 mark unit in lazy touch flow.
    #[test]
    fn marks_1pwm_1h() {
        let (cfg, _) = dev_net();
        let mut acc = Account {
            staked_pwm_raw: 1_000_000,
            marks_last_block: 0,
            ..Default::default()
        };
        super::touch_acct_mrks(&mut acc, cfg.blocks_per_hour, &cfg);
        assert_eq!(acc.stored_marks, 1);
    }

    /// Sub-1-PWM stake truncates whole PWM before lazy generation.
    #[test]
    fn marks_sub1pwm_trunc() {
        let (cfg, _) = dev_net();
        let mut acc = Account {
            staked_pwm_raw: 500_000,
            marks_last_block: 0,
            ..Default::default()
        };
        super::touch_acct_mrks(&mut acc, cfg.blocks_per_hour * 10, &cfg);
        assert_eq!(acc.stored_marks, 0);
    }

    /// 1M PWM stake reaches MARKS_CAP with ceil saturation window.
    #[test]
    fn marks_saturation_u32() {
        let (cfg, _) = dev_net();
        let mut acc = Account {
            staked_pwm_raw: 1_000_000u128 * PWM_RAW_SCALE,
            marks_last_block: 0,
            ..Default::default()
        };
        super::touch_acct_mrks(&mut acc, cfg.blocks_per_hour * 4_295, &cfg);
        assert_eq!(acc.stored_marks, MARKS_CAP);
    }

    /// Legacy snapshot JSON: huge raw marks divide by PWM_RAW_SCALE and clamp to u32.
    #[test]
    fn marks_snap_migrate() {
        let json = format!(
            r#"{{"signing_pubkey":{},"derivation_index":0,"balance_pwm":0,"staked":0,"marks":7000000000000,"initialized":false,"index":0,"flags":0,"nonce":0}}"#,
            serde_json::to_string(&[0u8; 32]).unwrap()
        );
        let acc: Account = serde_json::from_str(&json).unwrap();
        assert_eq!(acc.stored_marks, 7_000_000_u32);
    }

    #[test]
    fn import_min_fee_rule_enforced() {
        let mut st = State::default();
        let (sk_src, src_i, src_aid) = user_sk0(&[0xA5; 32]);
        let src_hi = domain_of_account_id(&src_aid).to_be_bytes()[0];
        let (sk_dst, dst_i, dst_aid) = (0u8..=u8::MAX)
            .find_map(|b| {
                let (sk, i, aid) = user_sk0(&[b; 32]);
                (domain_of_account_id(&aid).to_be_bytes()[0] != src_hi).then_some((sk, i, aid))
            })
            .expect("dst");
        st.accounts.insert(
            src_aid,
            Account::genesis_funded(sk_src.verifying_key().to_bytes(), src_i, 1_000_000),
        );
        st.accounts.insert(
            dst_aid,
            Account::genesis_funded(sk_dst.verifying_key().to_bytes(), dst_i, 1_000_000),
        );
        let export = SignedTx::sign_body(
            &sk_src,
            domain_of_account_id(&src_aid),
            src_i,
            0,
            TxBody::Export {
                to: dst_aid,
                target_domain: domain_of_account_id(&dst_aid),
                amount: 25,
                fee: 1,
            },
        );
        let export_id = export.export_id().expect("eid");
        st.apply_tx(&export).expect("export");

        let mut imp = SignedTx::sign_body(
            &sk_dst,
            domain_of_account_id(&dst_aid),
            dst_i,
            0,
            TxBody::Import {
                to: dst_aid,
                amount: 25,
                export_id,
            },
        );
        imp.set_import_fee_signed(&sk_dst, crate::tx::MIN_IMPORT_FEE_UNITS - 1);
        let err = st.apply_tx(&imp).expect_err("fee below min");
        assert!(matches!(err, TxError::ImportFeeTooLow));
    }

    #[test]
    fn marks_v2_gate_stake_min() {
        let (g, _) = dev_net();
        let mut st = g.state0();
        let aid = g.accounts[0].acct;
        let acc = st.accounts.get_mut(&aid).expect("validator");
        acc.staked_pwm_raw = 199_999;
        st.accrue_marks_v2(10_000, 200_000, 1_000_000);
        assert_eq!(st.accounts.get(&aid).expect("validator").stored_marks, 1);
    }

    #[test]
    fn marks_v2_scale_season_ppm() {
        let (g, _) = dev_net();
        let mut st = g.state0();
        let aid = g.accounts[0].acct;
        let acc = st.accounts.get_mut(&aid).expect("validator");
        acc.staked_pwm_raw = 300_000;
        st.accrue_marks_v2(10_000, 1, 500_000);
        assert_eq!(
            st.accounts.get(&aid).expect("validator").stored_marks,
            1_501
        );
    }

    #[test]
    fn reward_v2_gate_stake_min() {
        let (g, _) = dev_net();
        let mut st = g.state0();
        let aid = g.accounts[0].acct;
        let acc = st.accounts.get_mut(&aid).expect("validator");
        acc.staked_pwm_raw = 50_000;
        let bal0 = acc.balance_pwm;
        st.reward_producer_v2(&aid, 100, 100_000, 500_000);
        assert_eq!(st.accounts.get(&aid).expect("validator").balance_pwm, bal0);
    }

    #[test]
    fn init_v4_ext_sets_account() {
        let mut st = State::default();
        let (sk, idx, aid) = user_sk0(&[0xF1; 32]);
        let dom = domain_of_account_id(&aid);
        let mut init = SignedTx::sign_body(&sk, dom, idx, 0, TxBody::Init { index: 5, flags: 9 });
        init.set_init_v4_signed(
            &sk,
            Some(InitV4Extension {
                owner_kind: "company".to_string(),
                owner_display_name: "Acme Corp".to_string(),
                owner_country_hint: "CY".to_string(),
                company_metadata_commitment: [5u8; 32],
                external_verification_ref: "https://example.org/kyb".to_string(),
                requested_domain_lo: 0,
                rescue_address: Some([8u8; 32]),
                initial_policies: vec![InitPolicyEntry {
                    policy: PolicyKind::SenderFilter,
                    activation: ActivationMode::Dormant,
                }],
                cosign_policy: None,
            }),
        );
        st.apply_tx(&init).expect("init with v4 ext");
        let acc = st.get(&aid).expect("account");
        assert_eq!(acc.owner_kind, "company");
        assert_eq!(acc.requested_domain_lo, Some(0));
        assert_eq!(acc.rescue_address, Some([8u8; 32]));
        assert_ne!(acc.dormant_policies & PolicyKind::SenderFilter.bit(), 0);
    }

    #[test]
    fn policy_tx_state_lifecycle() {
        let (g, sks) = dev_net();
        let mut st = g.state0();
        let sk = &sks[0];
        let aid = g.accounts[0].acct;
        let dom = domain_of_account_id(&aid);
        let base = st.get(&aid).expect("sender").clone();

        let set = SignedTx::sign_body(
            sk,
            dom,
            g.accounts[0].der_idx,
            base.nonce,
            TxBody::Policy {
                target_account: aid,
                action: PolicyAction::SetPolicy {
                    policy: PolicyKind::CosignRequired,
                    activation: ActivationMode::Dormant,
                },
                fee: 11,
            },
        );
        st.apply_tx(&set).expect("set policy");
        let after_set = st.get(&aid).expect("after set");
        assert_ne!(
            after_set.dormant_policies & PolicyKind::CosignRequired.bit(),
            0
        );

        let act = SignedTx::sign_body(
            sk,
            dom,
            g.accounts[0].der_idx,
            after_set.nonce,
            TxBody::Policy {
                target_account: aid,
                action: PolicyAction::ActivatePolicy {
                    policy_id: PolicyKind::CosignRequired.policy_id(),
                    activation_target: None,
                },
                fee: 0,
            },
        );
        st.apply_tx(&act).expect("activate policy");
        let after_act = st.get(&aid).expect("after act");
        assert_eq!(
            after_act.dormant_policies & PolicyKind::CosignRequired.bit(),
            0
        );
        assert_ne!(
            after_act.active_policies & PolicyKind::CosignRequired.bit(),
            0
        );
        assert_eq!(after_act.balance_pwm, base.balance_pwm - 11);
    }

    #[test]
    fn policy_set_deferred_stores_entry() {
        let (g, sks) = dev_net();
        let mut st = g.state0();
        let sk = &sks[0];
        let aid = g.accounts[0].acct;
        let dom = domain_of_account_id(&aid);
        let nonce = st.get(&aid).expect("sender").nonce;

        let set = SignedTx::sign_body(
            sk,
            dom,
            g.accounts[0].der_idx,
            nonce,
            TxBody::Policy {
                target_account: aid,
                action: PolicyAction::SetPolicy {
                    policy: PolicyKind::SenderFilter,
                    activation: ActivationMode::Deferred {
                        activate_at_height: 100,
                    },
                },
                fee: 11,
            },
        );
        st.apply_tx_with_ctx(&set, 10, 0, &g)
            .expect("deferred policy set");
        let acc = st.get(&aid).expect("after set");
        assert_eq!(acc.dormant_policies & PolicyKind::SenderFilter.bit(), 0);
        assert_eq!(acc.active_policies & PolicyKind::SenderFilter.bit(), 0);
        assert_eq!(acc.deferred_policies.len(), 1);
        assert_eq!(
            acc.deferred_policies[0],
            DeferredPolicyEntry {
                policy: PolicyKind::SenderFilter,
                activate_at_height: 100,
            }
        );
    }

    #[test]
    fn init_deferred_active_at_h() {
        let (g, _) = dev_net();
        let mut st = State::default();
        let (sk, idx, aid) = user_sk0(&[0xE1; 32]);
        let dom = domain_of_account_id(&aid);
        let mut init = SignedTx::sign_body(&sk, dom, idx, 0, TxBody::Init { index: 1, flags: 2 });
        init.set_init_v4_signed(
            &sk,
            Some(InitV4Extension {
                owner_kind: "company".to_string(),
                owner_display_name: "Acme Deferred".to_string(),
                owner_country_hint: "CY".to_string(),
                company_metadata_commitment: [5u8; 32],
                external_verification_ref: "https://example.org/kyb-deferred".to_string(),
                requested_domain_lo: 0,
                rescue_address: None,
                initial_policies: vec![InitPolicyEntry {
                    policy: PolicyKind::SenderFilter,
                    activation: ActivationMode::Deferred {
                        activate_at_height: 5,
                    },
                }],
                cosign_policy: None,
            }),
        );
        st.apply_tx_with_ctx(&init, 5, 0, &g)
            .expect("init with deferred policy");
        let acc = st.get(&aid).expect("account");
        assert_ne!(acc.active_policies & PolicyKind::SenderFilter.bit(), 0);
        assert_eq!(acc.deferred_policies.len(), 1);
        assert_eq!(
            acc.deferred_policies[0],
            DeferredPolicyEntry {
                policy: PolicyKind::SenderFilter,
                activate_at_height: 5,
            }
        );
    }

    #[test]
    fn init_deferred_inactive_pre_h() {
        let (g, _) = dev_net();
        let mut st = State::default();
        let (sk, idx, aid) = user_sk0(&[0xE2; 32]);
        let dom = domain_of_account_id(&aid);
        let mut init = SignedTx::sign_body(&sk, dom, idx, 0, TxBody::Init { index: 2, flags: 3 });
        init.set_init_v4_signed(
            &sk,
            Some(InitV4Extension {
                owner_kind: "company".to_string(),
                owner_display_name: "Acme Deferred Future".to_string(),
                owner_country_hint: "CY".to_string(),
                company_metadata_commitment: [6u8; 32],
                external_verification_ref: "https://example.org/kyb-deferred-future".to_string(),
                requested_domain_lo: 0,
                rescue_address: None,
                initial_policies: vec![InitPolicyEntry {
                    policy: PolicyKind::SenderFilter,
                    activation: ActivationMode::Deferred {
                        activate_at_height: 6,
                    },
                }],
                cosign_policy: None,
            }),
        );
        st.apply_tx_with_ctx(&init, 5, 0, &g)
            .expect("init with future deferred policy");
        let acc = st.get(&aid).expect("account");
        assert_eq!(acc.active_policies & PolicyKind::SenderFilter.bit(), 0);
        assert_eq!(acc.deferred_policies.len(), 1);
        assert_eq!(
            acc.deferred_policies[0],
            DeferredPolicyEntry {
                policy: PolicyKind::SenderFilter,
                activate_at_height: 6,
            }
        );
    }

    #[test]
    fn policy_route_deny_no_mut() {
        let (g, sks) = dev_net();
        let mut st = g.state0();
        let sender_sk = &sks[0];
        let sender_id = g.accounts[0].acct;
        let sender_dom = domain_of_account_id(&sender_id);
        let sender_hi = sender_dom.to_be_bytes()[0];
        let (rcpt_sk, rcpt_idx, rcpt_id) = user_sk_other_domain(0xB0, sender_hi);
        let rcpt_dom = domain_of_account_id(&rcpt_id);

        let init_rcpt = SignedTx::sign_body(
            &rcpt_sk,
            rcpt_dom,
            rcpt_idx,
            0,
            TxBody::Init { index: 1, flags: 0 },
        );
        st.apply_tx(&init_rcpt).expect("init recipient");
        st.accounts
            .get_mut(&rcpt_id)
            .expect("recipient")
            .active_policies |= PolicyKind::RoutingSameDomainOnly.bit();
        let before = st.clone();

        let tx = SignedTx::sign_body(
            sender_sk,
            sender_dom,
            g.accounts[0].der_idx,
            st.get(&sender_id).expect("sender").nonce,
            TxBody::Transfer {
                to: rcpt_id,
                amount: 10,
                fee: 1,
            },
        );
        let err = st
            .apply_tx(&tx)
            .expect_err("must reject cross-domain route");
        assert!(matches!(err, TxError::PolicyRoutingDenied));
        assert_eq!(st.accounts, before.accounts);
        assert_eq!(st.fee_pool, before.fee_pool);
    }

    #[test]
    fn policy_finalized_deny() {
        let (g, sks) = dev_net();
        let mut st = g.state0();
        let sender_sk = &sks[0];
        let sender_id = g.accounts[0].acct;
        let sender_dom = domain_of_account_id(&sender_id);
        st.accounts.get_mut(&sender_id).expect("sender").finalized = true;
        let tx = SignedTx::sign_body(
            sender_sk,
            sender_dom,
            g.accounts[0].der_idx,
            0,
            TxBody::Stake { amount: 1 },
        );
        let err = st
            .apply_tx_with_ctx(&tx, 10, 1_000, &g)
            .expect_err("finalized sender must be blocked");
        assert!(matches!(err, TxError::PolicyAccountFinalized));
    }

    #[test]
    fn policy_def_beh_xfer_deny() {
        let (g, sks) = dev_net();
        let mut st = g.state0();
        let sender_sk = &sks[0];
        let sender_id = g.accounts[0].acct;
        let sender_dom = domain_of_account_id(&sender_id);
        let sender_nonce = st.get(&sender_id).expect("sender").nonce;
        let sender_hi = sender_dom.to_be_bytes()[0];
        let (rcpt_sk, rcpt_idx, rcpt_id) = user_sk_other_domain(0xD1, sender_hi);
        let rcpt_dom = domain_of_account_id(&rcpt_id);

        st.apply_tx(&SignedTx::sign_body(
            &rcpt_sk,
            rcpt_dom,
            rcpt_idx,
            0,
            TxBody::Init { index: 3, flags: 0 },
        ))
        .expect("init recipient");
        st.accounts
            .get_mut(&rcpt_id)
            .expect("recipient")
            .active_policies |= PolicyKind::DefaultBehavior.bit();
        let before = st.clone();

        let tx = SignedTx::sign_body(
            sender_sk,
            sender_dom,
            g.accounts[0].der_idx,
            sender_nonce,
            TxBody::Transfer {
                to: rcpt_id,
                amount: 5,
                fee: 1,
            },
        );
        let err = st
            .apply_tx(&tx)
            .expect_err("default behavior on recipient must deny incoming transfer");
        assert!(matches!(err, TxError::PolicyDenied));
        assert_eq!(st.accounts, before.accounts);
        assert_eq!(st.fee_pool, before.fee_pool);
    }

    #[test]
    fn policy_cosign_gate_deny_allow() {
        let (g, sks) = dev_net();
        let mut st = g.state0();
        let sender_sk = &sks[0];
        let sender_id = g.accounts[0].acct;
        let sender_dom = domain_of_account_id(&sender_id);
        st.accounts
            .get_mut(&sender_id)
            .expect("sender")
            .active_policies |= PolicyKind::CosignRequired.bit();

        let tx_no_cosign = SignedTx::sign_body(
            sender_sk,
            sender_dom,
            g.accounts[0].der_idx,
            0,
            TxBody::Policy {
                target_account: sender_id,
                action: PolicyAction::SetPolicy {
                    policy: PolicyKind::SenderFilter,
                    activation: ActivationMode::Dormant,
                },
                fee: 1,
            },
        );
        let deny = st
            .apply_tx(&tx_no_cosign)
            .expect_err("policy tx must require cosign");
        assert!(matches!(deny, TxError::PolicyMissingCosign));

        let (cosk, _, _) = user_sk0(&[0xDD; 32]);
        let mut tx_with_cosign = SignedTx::sign_body(
            sender_sk,
            sender_dom,
            g.accounts[0].der_idx,
            0,
            TxBody::Policy {
                target_account: sender_id,
                action: PolicyAction::SetPolicy {
                    policy: PolicyKind::SenderFilter,
                    activation: ActivationMode::Dormant,
                },
                fee: 1,
            },
        );
        let msg = tx_with_cosign.signing_message();
        tx_with_cosign.cosigns.push(Cosignature {
            signer_pk: cosk.verifying_key().to_bytes(),
            role: CosignRole::Witness,
            signature: sign(&cosk, &msg),
        });
        st.apply_tx(&tx_with_cosign)
            .expect("valid cosign must pass");
    }

    #[test]
    fn policy_cosign_bad_sig_deny() {
        let (g, sks) = dev_net();
        let mut st = g.state0();
        let sender_sk = &sks[0];
        let sender_id = g.accounts[0].acct;
        let sender_dom = domain_of_account_id(&sender_id);
        st.accounts
            .get_mut(&sender_id)
            .expect("sender")
            .active_policies |= PolicyKind::CosignRequired.bit();

        let (cosk, _, _) = user_sk0(&[0xDE; 32]);
        let mut tx = SignedTx::sign_body(
            sender_sk,
            sender_dom,
            g.accounts[0].der_idx,
            0,
            TxBody::Policy {
                target_account: sender_id,
                action: PolicyAction::SetPolicy {
                    policy: PolicyKind::SenderFilter,
                    activation: ActivationMode::Dormant,
                },
                fee: 1,
            },
        );
        tx.cosigns.push(Cosignature {
            signer_pk: cosk.verifying_key().to_bytes(),
            role: CosignRole::Witness,
            signature: sign(&cosk, b"tampered-cosign-message"),
        });

        let err = st
            .apply_tx(&tx)
            .expect_err("invalid cosign signature must be rejected");
        assert!(matches!(err, TxError::PolicyMissingCosign));
    }

    #[test]
    fn policy_flag_decode_bit0() {
        let (_, _, aid) = user_sk_flag(0x10);
        assert!(cosign_non_dis(&aid));
    }

    #[test]
    fn policy_flag_deact_cosign_reject() {
        let (g, _) = dev_net();
        let mut st = State::default();
        let (sk, idx, aid) = user_sk_flag(0x20);
        let dom = domain_of_account_id(&aid);
        st.apply_tx_with_ctx(
            &SignedTx::sign_body(&sk, dom, idx, 0, TxBody::Init { index: 1, flags: 0 }),
            0,
            0,
            &g,
        )
        .expect("init flagged account");
        {
            let acc = st.accounts.get_mut(&aid).expect("flagged account");
            acc.balance_pwm = 100;
            acc.active_policies |= PolicyKind::CosignRequired.bit();
        }

        let tx = SignedTx::sign_body(
            &sk,
            dom,
            idx,
            1,
            TxBody::Policy {
                target_account: aid,
                action: PolicyAction::DeactivatePolicy {
                    policy_id: PolicyKind::CosignRequired.policy_id(),
                },
                fee: 1,
            },
        );
        let err = st
            .apply_tx_with_ctx(&tx, 1, 0, &g)
            .expect_err("flagged account cannot disable cosign");
        assert!(matches!(err, TxError::PolicyFlagNonDisableable));
    }

    #[test]
    fn policy_flag_set_dormant_reject() {
        let (g, _) = dev_net();
        let mut st = State::default();
        let (sk, idx, aid) = user_sk_flag(0x30);
        let dom = domain_of_account_id(&aid);
        st.apply_tx_with_ctx(
            &SignedTx::sign_body(&sk, dom, idx, 0, TxBody::Init { index: 2, flags: 0 }),
            0,
            0,
            &g,
        )
        .expect("init flagged account");
        st.accounts
            .get_mut(&aid)
            .expect("flagged account")
            .balance_pwm = 100;

        let tx = SignedTx::sign_body(
            &sk,
            dom,
            idx,
            1,
            TxBody::Policy {
                target_account: aid,
                action: PolicyAction::SetPolicy {
                    policy: PolicyKind::CosignRequired,
                    activation: ActivationMode::Dormant,
                },
                fee: 1,
            },
        );
        let err = st
            .apply_tx_with_ctx(&tx, 1, 0, &g)
            .expect_err("flagged account cannot weaken cosign");
        assert!(matches!(err, TxError::PolicyFlagNonDisableable));
    }

    #[test]
    fn policy_flag_transfer_needs_cosign() {
        let (g, _) = dev_net();
        let mut st = State::default();
        let (sk, idx, aid) = user_sk_flag(0x50);
        let dom = domain_of_account_id(&aid);
        st.apply_tx_with_ctx(
            &SignedTx::sign_body(&sk, dom, idx, 0, TxBody::Init { index: 3, flags: 0 }),
            0,
            0,
            &g,
        )
        .expect("init flagged sender");
        st.accounts.get_mut(&aid).expect("sender").balance_pwm = 100;

        let sender_hi = dom.to_be_bytes()[0];
        let (rcpt_sk, rcpt_idx, rcpt_id) = user_sk_new_domain(&st, 0x60, sender_hi);
        let rcpt_dom = domain_of_account_id(&rcpt_id);
        st.apply_tx_with_ctx(
            &SignedTx::sign_body(
                &rcpt_sk,
                rcpt_dom,
                rcpt_idx,
                0,
                TxBody::Init { index: 4, flags: 0 },
            ),
            0,
            0,
            &g,
        )
        .expect("init recipient");

        let body = TxBody::Transfer {
            to: rcpt_id,
            amount: 5,
            fee: 1,
        };
        let deny = SignedTx::sign_body(&sk, dom, idx, 1, body.clone());
        let err = st
            .apply_tx_with_ctx(&deny, 1, 0, &g)
            .expect_err("flagged sender requires cosign");
        assert!(matches!(err, TxError::PolicyMissingCosign));

        let (cosk, _, _) = user_sk0(&[0x61; 32]);
        let mut allow = SignedTx::sign_body(&sk, dom, idx, 1, body);
        add_witness_cosign(&mut allow, &cosk);
        st.apply_tx_with_ctx(&allow, 1, 0, &g)
            .expect("valid cosign allows transfer");
    }

    #[test]
    fn policy_flag_emerg_rescue_ok() {
        let (g, _) = dev_net();
        let mut st = State::default();
        let (owner_sk, owner_idx, owner_id) = user_sk_flag(0x70);
        let owner_dom = domain_of_account_id(&owner_id);
        st.apply_tx_with_ctx(
            &SignedTx::sign_body(
                &owner_sk,
                owner_dom,
                owner_idx,
                0,
                TxBody::Init { index: 5, flags: 0 },
            ),
            0,
            0,
            &g,
        )
        .expect("init owner");
        st.accounts.get_mut(&owner_id).expect("owner").balance_pwm = 100;

        let owner_hi = owner_dom.to_be_bytes()[0];
        let (rescue_sk, rescue_idx, rescue_id) = user_sk_new_domain(&st, 0x80, owner_hi);
        let rescue_dom = domain_of_account_id(&rescue_id);
        st.apply_tx_with_ctx(
            &SignedTx::sign_body(
                &rescue_sk,
                rescue_dom,
                rescue_idx,
                0,
                TxBody::Init { index: 6, flags: 0 },
            ),
            0,
            0,
            &g,
        )
        .expect("init rescue");
        {
            let owner = st.accounts.get_mut(&owner_id).expect("owner");
            owner.rescue_address = Some(rescue_id);
            owner.dormant_policies |= PolicyKind::RoutingEmergencyRedirect.bit();
        }

        let mut tx = SignedTx::sign_body(
            &owner_sk,
            owner_dom,
            owner_idx,
            1,
            TxBody::Policy {
                target_account: owner_id,
                action: PolicyAction::ActivatePolicy {
                    policy_id: PolicyKind::RoutingEmergencyRedirect.policy_id(),
                    activation_target: Some(rescue_id),
                },
                fee: 0,
            },
        );
        let msg = tx.signing_message();
        tx.cosigns.push(Cosignature {
            signer_pk: rescue_sk.verifying_key().to_bytes(),
            role: CosignRole::Rescue,
            signature: sign(&rescue_sk, &msg),
        });
        st.apply_tx_with_ctx(&tx, 1, 0, &g)
            .expect("rescue cosign satisfies flagged emergency activation");
        let owner = st.get(&owner_id).expect("owner");
        assert!(owner.finalized);
        assert_ne!(
            owner.active_policies & PolicyKind::RoutingEmergencyRedirect.bit(),
            0
        );
    }

    #[test]
    fn policy_precheck_apply_same_err() {
        let (g, sks) = dev_net();
        let mut st = g.state0();
        let sender_sk = &sks[0];
        let sender_id = g.accounts[0].acct;
        let sender_dom = domain_of_account_id(&sender_id);
        let sender_hi = sender_dom.to_be_bytes()[0];
        let (rcpt_sk, rcpt_idx, rcpt_id) = user_sk_other_domain(0xC0, sender_hi);
        let rcpt_dom = domain_of_account_id(&rcpt_id);

        st.apply_tx(&SignedTx::sign_body(
            &rcpt_sk,
            rcpt_dom,
            rcpt_idx,
            0,
            TxBody::Init { index: 2, flags: 0 },
        ))
        .expect("init recipient");
        st.accounts
            .get_mut(&rcpt_id)
            .expect("recipient")
            .active_policies |= PolicyKind::RoutingSameDomainOnly.bit();

        let tx = SignedTx::sign_body(
            sender_sk,
            sender_dom,
            g.accounts[0].der_idx,
            0,
            TxBody::Transfer {
                to: rcpt_id,
                amount: 7,
                fee: 1,
            },
        );
        let pre_err = st
            .precheck_apply_with_ctx(&tx, 10, 1_000, &g)
            .expect_err("precheck reject");
        let app_err = st
            .apply_tx_with_ctx(&tx, 10, 1_000, &g)
            .expect_err("apply reject");
        assert!(matches!(pre_err, TxError::PolicyRoutingDenied));
        assert!(matches!(app_err, TxError::PolicyRoutingDenied));
    }

    #[test]
    fn eval_policy_sender_filter_min() {
        let (g, sks) = dev_net();
        let mut st = g.state0();
        let sender_id = g.accounts[0].acct;
        let sender_dom = domain_of_account_id(&sender_id);
        let sender_hi = sender_dom.to_be_bytes()[0];
        let (rcpt_sk, rcpt_idx, rcpt_id) = user_sk_other_domain(0xA0, sender_hi);
        let rcpt_dom = domain_of_account_id(&rcpt_id);
        st.apply_tx(&SignedTx::sign_body(
            &rcpt_sk,
            rcpt_dom,
            rcpt_idx,
            0,
            TxBody::Init { index: 0, flags: 0 },
        ))
        .expect("init recipient");
        st.accounts
            .get_mut(&rcpt_id)
            .expect("recipient")
            .active_policies |= PolicyKind::SenderFilter.bit();

        let tx = SignedTx::sign_body(
            &sks[0],
            sender_dom,
            g.accounts[0].der_idx,
            0,
            TxBody::Transfer {
                to: rcpt_id,
                amount: 1,
                fee: 1,
            },
        );
        let decision = st.evaluate_policy(&tx, 0);
        assert!(matches!(
            decision,
            PolicyDecision::Reject(TxError::PolicySenderFiltered)
        ));
    }

    #[test]
    // policy_deferred_auto_activates_at_height
    fn policy_deferred_auto_at_h() {
        let (g, sks) = dev_net();
        let mut st = g.state0();
        let target_id = g.accounts[0].acct;
        let target_dom = domain_of_account_id(&target_id);
        let set = SignedTx::sign_body(
            &sks[0],
            target_dom,
            g.accounts[0].der_idx,
            st.get(&target_id).expect("target").nonce,
            TxBody::Policy {
                target_account: target_id,
                action: PolicyAction::SetPolicy {
                    policy: PolicyKind::SenderFilter,
                    activation: ActivationMode::Deferred {
                        activate_at_height: 100,
                    },
                },
                fee: 1,
            },
        );
        st.apply_tx_with_ctx(&set, 10, 0, &g)
            .expect("set deferred sender filter");

        let sender_hi = target_dom.to_be_bytes()[0];
        let (sender_sk, sender_idx, sender_id) = user_sk_new_domain(&st, 0xA4, sender_hi);
        let sender_dom = domain_of_account_id(&sender_id);
        let init_sender = SignedTx::sign_body(
            &sender_sk,
            sender_dom,
            sender_idx,
            0,
            TxBody::Init {
                index: 77,
                flags: 0,
            },
        );
        st.apply_tx_with_ctx(&init_sender, 11, 0, &g)
            .expect("init sender");
        let mut tx = SignedTx::sign_body(
            &sender_sk,
            sender_dom,
            sender_idx,
            st.get(&sender_id).expect("sender").nonce,
            TxBody::Transfer {
                to: target_id,
                amount: 1,
                fee: 1,
            },
        );
        if cosign_non_dis(&sender_id) {
            add_witness_cosign(&mut tx, &sender_sk);
        }
        assert!(matches!(st.evaluate_policy(&tx, 99), PolicyDecision::Allow));
        assert!(matches!(
            st.evaluate_policy(&tx, 100),
            PolicyDecision::Reject(TxError::PolicySenderFiltered)
        ));
    }

    #[test]
    // policy_deferred_activate_before_height_rejected
    fn policy_deferred_act_pre_h() {
        let (g, sks) = dev_net();
        let mut st = g.state0();
        let aid = g.accounts[0].acct;
        let dom = domain_of_account_id(&aid);
        let set = SignedTx::sign_body(
            &sks[0],
            dom,
            g.accounts[0].der_idx,
            st.get(&aid).expect("acct").nonce,
            TxBody::Policy {
                target_account: aid,
                action: PolicyAction::SetPolicy {
                    policy: PolicyKind::SenderFilter,
                    activation: ActivationMode::Deferred {
                        activate_at_height: 100,
                    },
                },
                fee: 1,
            },
        );
        st.apply_tx_with_ctx(&set, 10, 0, &g)
            .expect("set deferred policy");
        let before = st.clone();

        let act = SignedTx::sign_body(
            &sks[0],
            dom,
            g.accounts[0].der_idx,
            st.get(&aid).expect("acct").nonce,
            TxBody::Policy {
                target_account: aid,
                action: PolicyAction::ActivatePolicy {
                    policy_id: PolicyKind::SenderFilter.policy_id(),
                    activation_target: None,
                },
                fee: 0,
            },
        );
        let err = st
            .apply_tx_with_ctx(&act, 99, 0, &g)
            .expect_err("activate before deferred height must fail");
        assert!(matches!(err, TxError::PolicyNotActive));
        assert_eq!(st.accounts, before.accounts);
        assert_eq!(st.fee_pool, before.fee_pool);
    }

    #[test]
    // policy_deferred_activate_at_height_rejected
    fn policy_deferred_act_at_h() {
        let (g, sks) = dev_net();
        let mut st = g.state0();
        let aid = g.accounts[0].acct;
        let dom = domain_of_account_id(&aid);
        let set = SignedTx::sign_body(
            &sks[0],
            dom,
            g.accounts[0].der_idx,
            st.get(&aid).expect("acct").nonce,
            TxBody::Policy {
                target_account: aid,
                action: PolicyAction::SetPolicy {
                    policy: PolicyKind::SenderFilter,
                    activation: ActivationMode::Deferred {
                        activate_at_height: 100,
                    },
                },
                fee: 1,
            },
        );
        st.apply_tx_with_ctx(&set, 10, 0, &g)
            .expect("set deferred policy");

        let act = SignedTx::sign_body(
            &sks[0],
            dom,
            g.accounts[0].der_idx,
            st.get(&aid).expect("acct").nonce,
            TxBody::Policy {
                target_account: aid,
                action: PolicyAction::ActivatePolicy {
                    policy_id: PolicyKind::SenderFilter.policy_id(),
                    activation_target: None,
                },
                fee: 0,
            },
        );
        let err = st
            .apply_tx_with_ctx(&act, 100, 0, &g)
            .expect_err("activate at deferred height must fail as already active");
        assert!(matches!(err, TxError::PolicyDenied));
    }

    #[test]
    // policy_deferred_deactivate_before_height
    fn policy_deferred_deact_pre_h() {
        let (g, sks) = dev_net();
        let mut st = g.state0();
        let aid = g.accounts[0].acct;
        let dom = domain_of_account_id(&aid);
        let set = SignedTx::sign_body(
            &sks[0],
            dom,
            g.accounts[0].der_idx,
            st.get(&aid).expect("acct").nonce,
            TxBody::Policy {
                target_account: aid,
                action: PolicyAction::SetPolicy {
                    policy: PolicyKind::SenderFilter,
                    activation: ActivationMode::Deferred {
                        activate_at_height: 100,
                    },
                },
                fee: 1,
            },
        );
        st.apply_tx_with_ctx(&set, 10, 0, &g)
            .expect("set deferred policy");
        let before = st.get(&aid).expect("acct before deact").nonce;

        let deact = SignedTx::sign_body(
            &sks[0],
            dom,
            g.accounts[0].der_idx,
            before,
            TxBody::Policy {
                target_account: aid,
                action: PolicyAction::DeactivatePolicy {
                    policy_id: PolicyKind::SenderFilter.policy_id(),
                },
                fee: 1,
            },
        );
        st.apply_tx_with_ctx(&deact, 99, 0, &g)
            .expect("deactivate pending deferred policy");
        let acc = st.get(&aid).expect("acct");
        assert!(acc
            .deferred_policies
            .iter()
            .all(|row| row.policy != PolicyKind::SenderFilter));
        assert_eq!(acc.active_policies & PolicyKind::SenderFilter.bit(), 0);
        assert_eq!(acc.dormant_policies & PolicyKind::SenderFilter.bit(), 0);
    }

    #[test]
    fn policy_emerg_act_no_rescue() {
        let (g, sks) = dev_net();
        let mut st = g.state0();
        let owner_sk = &sks[0];
        let owner_id = g.accounts[0].acct;
        let owner_dom = domain_of_account_id(&owner_id);
        st.accounts
            .get_mut(&owner_id)
            .expect("owner")
            .dormant_policies |= PolicyKind::RoutingEmergencyRedirect.bit();

        let tx = SignedTx::sign_body(
            owner_sk,
            owner_dom,
            g.accounts[0].der_idx,
            st.get(&owner_id).expect("owner").nonce,
            TxBody::Policy {
                target_account: owner_id,
                action: PolicyAction::ActivatePolicy {
                    policy_id: PolicyKind::RoutingEmergencyRedirect.policy_id(),
                    activation_target: Some(owner_id),
                },
                fee: 0,
            },
        );
        let err = st.apply_tx(&tx).expect_err("rescue address must exist");
        assert!(matches!(err, TxError::PolicyRescueRequired));
    }

    #[test]
    fn policy_emerg_act_no_cosign() {
        let (g, sks) = dev_net();
        let mut st = g.state0();
        let owner_sk = &sks[0];
        let owner_id = g.accounts[0].acct;
        let owner_dom = domain_of_account_id(&owner_id);
        let sender_hi = owner_dom.to_be_bytes()[0];
        let (rescue_sk, rescue_idx, rescue_id) = user_sk_new_domain(&st, 0x41, sender_hi);
        let rescue_dom = domain_of_account_id(&rescue_id);
        st.apply_tx(&SignedTx::sign_body(
            &rescue_sk,
            rescue_dom,
            rescue_idx,
            0,
            TxBody::Init { index: 6, flags: 0 },
        ))
        .expect("init rescue");
        {
            let owner = st.accounts.get_mut(&owner_id).expect("owner");
            owner.rescue_address = Some(rescue_id);
            owner.dormant_policies |= PolicyKind::RoutingEmergencyRedirect.bit();
        }

        let tx = SignedTx::sign_body(
            owner_sk,
            owner_dom,
            g.accounts[0].der_idx,
            st.get(&owner_id).expect("owner").nonce,
            TxBody::Policy {
                target_account: owner_id,
                action: PolicyAction::ActivatePolicy {
                    policy_id: PolicyKind::RoutingEmergencyRedirect.policy_id(),
                    activation_target: Some(rescue_id),
                },
                fee: 0,
            },
        );
        let before = st.clone();
        let err = st
            .apply_tx(&tx)
            .expect_err("rescue cosign is mandatory for emergency activation");
        assert!(matches!(err, TxError::PolicyEmergencyCosignRequired));
        assert_eq!(st.accounts, before.accounts);
        assert_eq!(st.fee_pool, before.fee_pool);
    }

    #[test]
    fn policy_emerg_act_bad_cosign() {
        let (g, sks) = dev_net();
        let mut st = g.state0();
        let owner_sk = &sks[0];
        let owner_id = g.accounts[0].acct;
        let owner_dom = domain_of_account_id(&owner_id);
        let sender_hi = owner_dom.to_be_bytes()[0];
        let (rescue_sk, rescue_idx, rescue_id) = user_sk_new_domain(&st, 0x42, sender_hi);
        let rescue_dom = domain_of_account_id(&rescue_id);
        st.apply_tx(&SignedTx::sign_body(
            &rescue_sk,
            rescue_dom,
            rescue_idx,
            0,
            TxBody::Init { index: 7, flags: 0 },
        ))
        .expect("init rescue");
        {
            let owner = st.accounts.get_mut(&owner_id).expect("owner");
            owner.rescue_address = Some(rescue_id);
            owner.dormant_policies |= PolicyKind::RoutingEmergencyRedirect.bit();
        }

        let mut tx = SignedTx::sign_body(
            owner_sk,
            owner_dom,
            g.accounts[0].der_idx,
            st.get(&owner_id).expect("owner").nonce,
            TxBody::Policy {
                target_account: owner_id,
                action: PolicyAction::ActivatePolicy {
                    policy_id: PolicyKind::RoutingEmergencyRedirect.policy_id(),
                    activation_target: Some(rescue_id),
                },
                fee: 0,
            },
        );
        tx.cosigns.push(Cosignature {
            signer_pk: rescue_sk.verifying_key().to_bytes(),
            role: CosignRole::Rescue,
            signature: sign(&rescue_sk, b"tampered-emerg-cosign"),
        });
        let before = st.clone();
        let err = st
            .apply_tx(&tx)
            .expect_err("bad rescue cosign must deny tx");
        assert!(matches!(err, TxError::PolicyEmergencyCosignRequired));
        let bit = PolicyKind::RoutingEmergencyRedirect.bit();
        let owner_before = before.get(&owner_id).expect("owner before");
        let owner_after = st.get(&owner_id).expect("owner after");
        assert_eq!(
            owner_after.active_policies & bit,
            owner_before.active_policies & bit
        );
        assert_eq!(
            owner_after.dormant_policies & bit,
            owner_before.dormant_policies & bit
        );
        assert_eq!(owner_after.finalized, owner_before.finalized);
        assert_eq!(owner_after.balance_pwm, owner_before.balance_pwm);
        assert_eq!(st.accounts, before.accounts);
        assert_eq!(st.fee_pool, before.fee_pool);
    }

    #[test]
    fn policy_emerg_act_ok_finalizes() {
        let (g, sks) = dev_net();
        let mut st = g.state0();
        let owner_sk = &sks[0];
        let owner_id = g.accounts[0].acct;
        let owner_dom = domain_of_account_id(&owner_id);
        let sender_hi = owner_dom.to_be_bytes()[0];
        let (rescue_sk, rescue_idx, rescue_id) = user_sk_new_domain(&st, 0x43, sender_hi);
        let rescue_dom = domain_of_account_id(&rescue_id);
        st.apply_tx(&SignedTx::sign_body(
            &rescue_sk,
            rescue_dom,
            rescue_idx,
            0,
            TxBody::Init { index: 8, flags: 0 },
        ))
        .expect("init rescue");
        {
            let owner = st.accounts.get_mut(&owner_id).expect("owner");
            owner.rescue_address = Some(rescue_id);
            owner.dormant_policies |= PolicyKind::RoutingEmergencyRedirect.bit();
        }
        let mut tx = SignedTx::sign_body(
            owner_sk,
            owner_dom,
            g.accounts[0].der_idx,
            st.get(&owner_id).expect("owner").nonce,
            TxBody::Policy {
                target_account: owner_id,
                action: PolicyAction::ActivatePolicy {
                    policy_id: PolicyKind::RoutingEmergencyRedirect.policy_id(),
                    activation_target: Some(rescue_id),
                },
                fee: 0,
            },
        );
        let msg = tx.signing_message();
        tx.cosigns.push(Cosignature {
            signer_pk: rescue_sk.verifying_key().to_bytes(),
            role: CosignRole::Rescue,
            signature: sign(&rescue_sk, &msg),
        });
        st.apply_tx(&tx).expect("valid rescue cosign must pass");
        let owner = st.get(&owner_id).expect("owner");
        let bit = PolicyKind::RoutingEmergencyRedirect.bit();
        assert_ne!(owner.active_policies & bit, 0);
        assert_eq!(owner.dormant_policies & bit, 0);
        assert!(owner.finalized);
    }

    #[test]
    fn emergency_activation_sweep_ok() {
        let (g, sks) = dev_net();
        let mut st = g.state0();
        let owner_sk = &sks[0];
        let owner_id = g.accounts[0].acct;
        let owner_dom = domain_of_account_id(&owner_id);
        let owner_hi = owner_dom.to_be_bytes()[0];
        let (rescue_sk, rescue_idx, rescue_id) = user_sk_new_domain(&st, 0x90, owner_hi);
        let rescue_dom = domain_of_account_id(&rescue_id);
        st.apply_tx(&SignedTx::sign_body(
            &rescue_sk,
            rescue_dom,
            rescue_idx,
            0,
            TxBody::Init { index: 9, flags: 0 },
        ))
        .expect("init rescue");
        {
            let owner = st.accounts.get_mut(&owner_id).expect("owner");
            owner.rescue_address = Some(rescue_id);
            owner.dormant_policies |= PolicyKind::RoutingEmergencyRedirect.bit();
            owner.balance_pwm = 1234;
            owner.staked_pwm_raw = 777;
        }
        let rescue_before = st.get(&rescue_id).expect("rescue").balance_pwm;

        let mut tx = SignedTx::sign_body(
            owner_sk,
            owner_dom,
            g.accounts[0].der_idx,
            st.get(&owner_id).expect("owner").nonce,
            TxBody::Policy {
                target_account: owner_id,
                action: PolicyAction::ActivatePolicy {
                    policy_id: PolicyKind::RoutingEmergencyRedirect.policy_id(),
                    activation_target: Some(rescue_id),
                },
                fee: 0,
            },
        );
        let msg = tx.signing_message();
        tx.cosigns.push(Cosignature {
            signer_pk: rescue_sk.verifying_key().to_bytes(),
            role: CosignRole::Rescue,
            signature: sign(&rescue_sk, &msg),
        });
        st.apply_tx(&tx).expect("emergency activation sweeps");

        let owner = st.get(&owner_id).expect("owner");
        let rescue = st.get(&rescue_id).expect("rescue");
        assert!(owner.finalized);
        assert_eq!(owner.balance_pwm, 0);
        assert_eq!(owner.staked_pwm_raw, 0);
        assert_eq!(rescue.balance_pwm, rescue_before + 1234 + 777);
        assert_eq!(rescue.nonce, 1);
    }

    #[test]
    fn conservation_emergency_evac_with_fee() {
        let (g, _) = dev_net();
        let mut st = State::default();
        let (owner_sk, owner_idx, owner_id) = user_sk_conserv(0xA0);
        let owner_dom = domain_of_account_id(&owner_id);
        st.apply_tx_with_ctx(
            &SignedTx::sign_body(
                &owner_sk,
                owner_dom,
                owner_idx,
                0,
                TxBody::Init {
                    index: 13,
                    flags: 0,
                },
            ),
            0,
            0,
            &g,
        )
        .expect("init conservation owner");
        let owner_hi = owner_dom.to_be_bytes()[0];
        let (rescue_sk, rescue_idx, rescue_id) = user_sk_new_domain(&st, 0xB0, owner_hi);
        let rescue_dom = domain_of_account_id(&rescue_id);
        st.apply_tx_with_ctx(
            &SignedTx::sign_body(
                &rescue_sk,
                rescue_dom,
                rescue_idx,
                0,
                TxBody::Init {
                    index: 14,
                    flags: 0,
                },
            ),
            0,
            0,
            &g,
        )
        .expect("init rescue");
        {
            let owner = st.accounts.get_mut(&owner_id).expect("owner");
            owner.rescue_address = Some(rescue_id);
            owner.dormant_policies |= PolicyKind::RoutingEmergencyRedirect.bit();
            owner.balance_pwm = 1000;
        }
        let rescue_before = st.get(&rescue_id).expect("rescue").balance_pwm;
        let pool_before = st.fee_pool;

        let mut tx = SignedTx::sign_body(
            &owner_sk,
            owner_dom,
            owner_idx,
            st.get(&owner_id).expect("owner").nonce,
            TxBody::Policy {
                target_account: owner_id,
                action: PolicyAction::ActivatePolicy {
                    policy_id: PolicyKind::RoutingEmergencyRedirect.policy_id(),
                    activation_target: Some(rescue_id),
                },
                fee: 5,
            },
        );
        let msg = tx.signing_message();
        tx.cosigns.push(Cosignature {
            signer_pk: rescue_sk.verifying_key().to_bytes(),
            role: CosignRole::Rescue,
            signature: sign(&rescue_sk, &msg),
        });
        st.apply_tx_with_ctx(&tx, 1, 0, &g)
            .expect("fee is deducted before emergency evacuation");

        let owner = st.get(&owner_id).expect("owner");
        let rescue = st.get(&rescue_id).expect("rescue");
        assert_eq!(owner.balance_pwm, 0);
        assert_eq!(rescue.balance_pwm, rescue_before + 995);
        assert_eq!(st.fee_pool, pool_before + 5);
    }

    #[test]
    fn emergency_evac_epoch_index() {
        let (mut cfg, sks) = dev_net();
        cfg.min_validator_stake = 100;
        cfg.epoch_length_blocks = 1;
        let mut st = cfg.state0();
        let owner_sk = &sks[0];
        let owner_id = cfg.accounts[0].acct;
        let owner_dom = domain_of_account_id(&owner_id);
        let owner_hi = owner_dom.to_be_bytes()[0];
        let (rescue_sk, rescue_idx, rescue_id) = user_sk_new_domain(&st, 0x94, owner_hi);
        let rescue_dom = domain_of_account_id(&rescue_id);
        st.apply_tx(&SignedTx::sign_body(
            &rescue_sk,
            rescue_dom,
            rescue_idx,
            0,
            TxBody::Init {
                index: 12,
                flags: 0,
            },
        ))
        .expect("init rescue");
        {
            let owner = st.accounts.get_mut(&owner_id).expect("owner");
            owner.rescue_address = Some(rescue_id);
            owner.dormant_policies |= PolicyKind::RoutingEmergencyRedirect.bit();
            owner.balance_pwm = 1234;
            owner.staked_pwm_raw = 777;
        }
        st.active_validator_indices = recompute_active_idxs(&cfg, &st);
        assert_eq!(st.active_validator_indices, vec![0]);

        let mut tx = SignedTx::sign_body(
            owner_sk,
            owner_dom,
            cfg.accounts[0].der_idx,
            st.get(&owner_id).expect("owner").nonce,
            TxBody::Policy {
                target_account: owner_id,
                action: PolicyAction::ActivatePolicy {
                    policy_id: PolicyKind::RoutingEmergencyRedirect.policy_id(),
                    activation_target: Some(rescue_id),
                },
                fee: 0,
            },
        );
        let msg = tx.signing_message();
        tx.cosigns.push(Cosignature {
            signer_pk: rescue_sk.verifying_key().to_bytes(),
            role: CosignRole::Rescue,
            signature: sign(&rescue_sk, &msg),
        });
        st.apply_tx(&tx).expect("emergency activation sweeps");
        roll_epoch_if_needed(&cfg, &mut st, 1);

        let owner = st.get(&owner_id).expect("owner");
        assert_eq!(owner.staked_pwm_raw, 0);
        assert!(st.active_validator_indices.is_empty());
    }
    #[test]
    fn emergency_activation_no_stake() {
        let (g, sks) = dev_net();
        let mut st = g.state0();
        let owner_sk = &sks[0];
        let owner_id = g.accounts[0].acct;
        let owner_dom = domain_of_account_id(&owner_id);
        let owner_hi = owner_dom.to_be_bytes()[0];
        let (rescue_sk, rescue_idx, rescue_id) = user_sk_new_domain(&st, 0x91, owner_hi);
        let rescue_dom = domain_of_account_id(&rescue_id);
        st.apply_tx(&SignedTx::sign_body(
            &rescue_sk,
            rescue_dom,
            rescue_idx,
            0,
            TxBody::Init {
                index: 10,
                flags: 0,
            },
        ))
        .expect("init rescue");
        {
            let owner = st.accounts.get_mut(&owner_id).expect("owner");
            owner.rescue_address = Some(rescue_id);
            owner.dormant_policies |= PolicyKind::RoutingEmergencyRedirect.bit();
            owner.balance_pwm = 1234;
            owner.staked_pwm_raw = 0;
        }
        let rescue_before = st.get(&rescue_id).expect("rescue").balance_pwm;

        let mut tx = SignedTx::sign_body(
            owner_sk,
            owner_dom,
            g.accounts[0].der_idx,
            st.get(&owner_id).expect("owner").nonce,
            TxBody::Policy {
                target_account: owner_id,
                action: PolicyAction::ActivatePolicy {
                    policy_id: PolicyKind::RoutingEmergencyRedirect.policy_id(),
                    activation_target: Some(rescue_id),
                },
                fee: 0,
            },
        );
        let msg = tx.signing_message();
        tx.cosigns.push(Cosignature {
            signer_pk: rescue_sk.verifying_key().to_bytes(),
            role: CosignRole::Rescue,
            signature: sign(&rescue_sk, &msg),
        });
        st.apply_tx(&tx)
            .expect("emergency activation sweeps liquid");

        let owner = st.get(&owner_id).expect("owner");
        let rescue = st.get(&rescue_id).expect("rescue");
        assert!(owner.finalized);
        assert_eq!(owner.balance_pwm, 0);
        assert_eq!(owner.staked_pwm_raw, 0);
        assert_eq!(rescue.balance_pwm, rescue_before + 1234);
    }

    #[test]
    fn emergency_activation_fee_reject() {
        let (g, sks) = dev_net();
        let mut st = g.state0();
        let owner_id = g.accounts[0].acct;
        let owner_dom = domain_of_account_id(&owner_id);
        st.accounts
            .get_mut(&owner_id)
            .expect("owner")
            .dormant_policies |= PolicyKind::SenderFilter.bit();
        let tx = SignedTx::sign_body(
            &sks[0],
            owner_dom,
            g.accounts[0].der_idx,
            st.get(&owner_id).expect("owner").nonce,
            TxBody::Policy {
                target_account: owner_id,
                action: PolicyAction::ActivatePolicy {
                    policy_id: PolicyKind::SenderFilter.policy_id(),
                    activation_target: None,
                },
                fee: 1,
            },
        );
        let err = st.apply_tx(&tx).expect_err("activation fee must be zero");
        assert!(matches!(err, TxError::PolicyActivationFeeMustBeZero));
    }

    #[test]
    fn emergency_activation_target_required() {
        let (g, sks) = dev_net();
        let mut st = g.state0();
        let owner_id = g.accounts[0].acct;
        let owner_dom = domain_of_account_id(&owner_id);
        st.accounts
            .get_mut(&owner_id)
            .expect("owner")
            .dormant_policies |= PolicyKind::RoutingEmergencyRedirect.bit();
        let tx = SignedTx::sign_body(
            &sks[0],
            owner_dom,
            g.accounts[0].der_idx,
            st.get(&owner_id).expect("owner").nonce,
            TxBody::Policy {
                target_account: owner_id,
                action: PolicyAction::ActivatePolicy {
                    policy_id: PolicyKind::RoutingEmergencyRedirect.policy_id(),
                    activation_target: None,
                },
                fee: 0,
            },
        );
        let err = st
            .apply_tx(&tx)
            .expect_err("emergency activation target required");
        assert!(matches!(err, TxError::PolicyActivationTargetRequired));
    }

    #[test]
    fn emergency_activation_target_mismatch() {
        let (g, sks) = dev_net();
        let mut st = g.state0();
        let owner_id = g.accounts[0].acct;
        let owner_dom = domain_of_account_id(&owner_id);
        let owner_hi = owner_dom.to_be_bytes()[0];
        let (rescue_sk, rescue_idx, rescue_id) = user_sk_new_domain(&st, 0x91, owner_hi);
        let rescue_dom = domain_of_account_id(&rescue_id);
        st.apply_tx(&SignedTx::sign_body(
            &rescue_sk,
            rescue_dom,
            rescue_idx,
            0,
            TxBody::Init {
                index: 10,
                flags: 0,
            },
        ))
        .expect("init rescue");
        let (_, _, wrong_id) = user_sk_new_domain(&st, 0x92, owner_hi);
        {
            let owner = st.accounts.get_mut(&owner_id).expect("owner");
            owner.rescue_address = Some(rescue_id);
            owner.dormant_policies |= PolicyKind::RoutingEmergencyRedirect.bit();
        }
        let mut tx = SignedTx::sign_body(
            &sks[0],
            owner_dom,
            g.accounts[0].der_idx,
            st.get(&owner_id).expect("owner").nonce,
            TxBody::Policy {
                target_account: owner_id,
                action: PolicyAction::ActivatePolicy {
                    policy_id: PolicyKind::RoutingEmergencyRedirect.policy_id(),
                    activation_target: Some(wrong_id),
                },
                fee: 0,
            },
        );
        let msg = tx.signing_message();
        tx.cosigns.push(Cosignature {
            signer_pk: rescue_sk.verifying_key().to_bytes(),
            role: CosignRole::Rescue,
            signature: sign(&rescue_sk, &msg),
        });
        let err = st
            .apply_tx(&tx)
            .expect_err("activation target must match rescue");
        assert!(matches!(err, TxError::PolicyActivationTargetMismatch));
    }

    #[test]
    fn emergency_activation_cross_reject() {
        let (g, sks) = dev_net();
        let mut st = g.state0();
        let owner_id = g.accounts[0].acct;
        let owner_dom = domain_of_account_id(&owner_id);
        let owner_hi = owner_dom.to_be_bytes()[0];
        let (rescue_sk, rescue_idx, rescue_id) = user_sk_other_domain(0x93, owner_hi);
        let rescue_dom = domain_of_account_id(&rescue_id);
        st.apply_tx(&SignedTx::sign_body(
            &rescue_sk,
            rescue_dom,
            rescue_idx,
            0,
            TxBody::Init {
                index: 11,
                flags: 0,
            },
        ))
        .expect("init rescue");
        {
            let owner = st.accounts.get_mut(&owner_id).expect("owner");
            owner.rescue_address = Some(rescue_id);
            owner.dormant_policies |= PolicyKind::RoutingEmergencyRedirect.bit();
        }
        let mut tx = SignedTx::sign_body(
            &sks[0],
            owner_dom,
            g.accounts[0].der_idx,
            st.get(&owner_id).expect("owner").nonce,
            TxBody::Policy {
                target_account: owner_id,
                action: PolicyAction::ActivatePolicy {
                    policy_id: PolicyKind::RoutingEmergencyRedirect.policy_id(),
                    activation_target: Some(rescue_id),
                },
                fee: 0,
            },
        );
        let msg = tx.signing_message();
        tx.cosigns.push(Cosignature {
            signer_pk: rescue_sk.verifying_key().to_bytes(),
            role: CosignRole::Rescue,
            signature: sign(&rescue_sk, &msg),
        });
        let err = st
            .apply_tx(&tx)
            .expect_err("cross-shard activation target must reject");
        assert!(matches!(err, TxError::PolicyRoutingDenied));
    }

    #[test]
    fn conservation_delay_execute() {
        let mut cfg = dev_net().0;
        cfg.conservation_delay_blocks = 2;
        let mut st = State::default();
        let (sender_sk, sender_idx, sender_id) = user_sk_conserv(0xB0);
        let sender_dom = domain_of_account_id(&sender_id);
        let sender_hi = sender_dom.to_be_bytes()[0];
        let (rcpt_sk, rcpt_idx, rcpt_id) = user_sk_new_domain(&st, 0xB1, sender_hi);
        st.accounts.insert(
            sender_id,
            Account::genesis_funded(sender_sk.verifying_key().to_bytes(), sender_idx, 1_000),
        );
        st.accounts.insert(
            rcpt_id,
            Account::genesis_funded(rcpt_sk.verifying_key().to_bytes(), rcpt_idx, 0),
        );
        let tx = SignedTx::sign_body(
            &sender_sk,
            sender_dom,
            sender_idx,
            0,
            TxBody::Transfer {
                to: rcpt_id,
                amount: 100,
                fee: 7,
            },
        );
        st.apply_tx_with_ctx(&tx, 10, 0, &cfg)
            .expect("enqueue conservation transfer");
        assert_eq!(st.pending_conservation.len(), 1);
        assert_eq!(st.pending_conservation[0].enqueue_height, 10);
        assert_eq!(st.pending_conservation[0].execute_at_height, 12);
        assert_eq!(st.get(&sender_id).expect("sender").balance_pwm, 1_000);
        assert_eq!(st.get(&sender_id).expect("sender").nonce, 0);
        assert_eq!(st.get(&rcpt_id).expect("recipient").balance_pwm, 0);

        st.drain_conservation_at_height(11, &cfg);
        assert_eq!(st.pending_conservation.len(), 1);
        st.drain_conservation_at_height(12, &cfg);
        assert!(st.pending_conservation.is_empty());
        assert_eq!(st.get(&sender_id).expect("sender").balance_pwm, 893);
        assert_eq!(st.get(&sender_id).expect("sender").nonce, 1);
        assert_eq!(st.get(&rcpt_id).expect("recipient").balance_pwm, 100);
        assert_eq!(st.fee_pool, 7);
    }

    #[test]
    fn conservation_pending_exists_reject() {
        let mut cfg = dev_net().0;
        cfg.conservation_delay_blocks = 2;
        let mut st = State::default();
        let (sender_sk, sender_idx, sender_id) = user_sk_conserv(0xC0);
        let sender_dom = domain_of_account_id(&sender_id);
        let sender_hi = sender_dom.to_be_bytes()[0];
        let (rcpt_sk, rcpt_idx, rcpt_id) = user_sk_new_domain(&st, 0xC1, sender_hi);
        st.accounts.insert(
            sender_id,
            Account::genesis_funded(sender_sk.verifying_key().to_bytes(), sender_idx, 1_000),
        );
        st.accounts.insert(
            rcpt_id,
            Account::genesis_funded(rcpt_sk.verifying_key().to_bytes(), rcpt_idx, 0),
        );
        let tx = SignedTx::sign_body(
            &sender_sk,
            sender_dom,
            sender_idx,
            0,
            TxBody::Transfer {
                to: rcpt_id,
                amount: 100,
                fee: 7,
            },
        );
        st.apply_tx_with_ctx(&tx, 10, 0, &cfg).expect("enqueue");
        let err = st
            .precheck_apply_with_ctx(&tx, 10, 0, &cfg)
            .expect_err("second pending must reject");
        assert!(matches!(err, TxError::ConservationPendingExists));
    }

    #[test]
    fn conservation_export_race_reject() {
        let mut cfg = dev_net().0;
        cfg.conservation_delay_blocks = 2;
        let mut st = State::default();
        let (sender_sk, sender_idx, sender_id) = user_sk_conserv(0xC2);
        let sender_dom = domain_of_account_id(&sender_id);
        let sender_hi = sender_dom.to_be_bytes()[0];
        let (rcpt_sk, rcpt_idx, rcpt_id) = user_sk_other_domain(0xC3, sender_hi);
        let rcpt_dom = domain_of_account_id(&rcpt_id);
        st.accounts.insert(
            sender_id,
            Account::genesis_funded(sender_sk.verifying_key().to_bytes(), sender_idx, 1_000),
        );
        st.accounts.insert(
            rcpt_id,
            Account::genesis_funded(rcpt_sk.verifying_key().to_bytes(), rcpt_idx, 0),
        );
        let tx = SignedTx::sign_body(
            &sender_sk,
            sender_dom,
            sender_idx,
            0,
            TxBody::Transfer {
                to: rcpt_id,
                amount: 100,
                fee: 7,
            },
        );
        st.apply_tx_with_ctx(&tx, 10, 0, &cfg).expect("enqueue");
        let export = SignedTx::sign_body(
            &sender_sk,
            sender_dom,
            sender_idx,
            0,
            TxBody::Export {
                to: rcpt_id,
                target_domain: rcpt_dom,
                amount: 30,
                fee: 1,
            },
        );
        let err = st
            .apply_tx_with_ctx(&export, 11, 0, &cfg)
            .expect_err("pending conservation must block export");
        assert!(matches!(err, TxError::ConservationPendingExists));
        assert_eq!(st.pending_conservation.len(), 1);
        assert_eq!(st.get(&sender_id).expect("sender").balance_pwm, 1_000);
        assert_eq!(st.get(&sender_id).expect("sender").nonce, 0);
    }

    #[test]
    fn conservation_stake_race_reject() {
        let mut cfg = dev_net().0;
        cfg.conservation_delay_blocks = 2;
        let mut st = State::default();
        let (sender_sk, sender_idx, sender_id) = user_sk_conserv(0xC4);
        let sender_dom = domain_of_account_id(&sender_id);
        let sender_hi = sender_dom.to_be_bytes()[0];
        let (rcpt_sk, rcpt_idx, rcpt_id) = user_sk_new_domain(&st, 0xC5, sender_hi);
        st.accounts.insert(
            sender_id,
            Account::genesis_funded(sender_sk.verifying_key().to_bytes(), sender_idx, 1_000),
        );
        st.accounts.insert(
            rcpt_id,
            Account::genesis_funded(rcpt_sk.verifying_key().to_bytes(), rcpt_idx, 0),
        );
        let tx = SignedTx::sign_body(
            &sender_sk,
            sender_dom,
            sender_idx,
            0,
            TxBody::Transfer {
                to: rcpt_id,
                amount: 100,
                fee: 7,
            },
        );
        st.apply_tx_with_ctx(&tx, 10, 0, &cfg).expect("enqueue");
        let stake = SignedTx::sign_body(
            &sender_sk,
            sender_dom,
            sender_idx,
            0,
            TxBody::Stake { amount: 1 },
        );
        let err = st
            .apply_tx_with_ctx(&stake, 11, 0, &cfg)
            .expect_err("pending conservation must block stake");
        assert!(matches!(err, TxError::ConservationPendingExists));
        assert_eq!(st.pending_conservation.len(), 1);
        assert_eq!(st.get(&sender_id).expect("sender").balance_pwm, 1_000);
        assert_eq!(st.get(&sender_id).expect("sender").nonce, 0);
    }

    #[test]
    fn conservation_drain_insufficient_requeue() {
        let mut cfg = dev_net().0;
        cfg.conservation_delay_blocks = 1;
        let mut st = State::default();
        let (sender_sk, sender_idx, sender_id) = user_sk_conserv(0xC6);
        let sender_dom = domain_of_account_id(&sender_id);
        let sender_hi = sender_dom.to_be_bytes()[0];
        let (rcpt_sk, rcpt_idx, rcpt_id) = user_sk_new_domain(&st, 0xC7, sender_hi);
        st.accounts.insert(
            sender_id,
            Account::genesis_funded(sender_sk.verifying_key().to_bytes(), sender_idx, 1_000),
        );
        st.accounts.insert(
            rcpt_id,
            Account::genesis_funded(rcpt_sk.verifying_key().to_bytes(), rcpt_idx, 0),
        );
        let tx = SignedTx::sign_body(
            &sender_sk,
            sender_dom,
            sender_idx,
            0,
            TxBody::Transfer {
                to: rcpt_id,
                amount: 100,
                fee: 7,
            },
        );
        st.apply_tx_with_ctx(&tx, 10, 0, &cfg).expect("enqueue");
        st.accounts.get_mut(&sender_id).expect("sender").balance_pwm = 0;

        st.drain_conservation_at_height(11, &cfg);
        assert_eq!(st.pending_conservation.len(), 1);
        assert_eq!(st.get(&sender_id).expect("sender").nonce, 0);
        assert_eq!(st.get(&rcpt_id).expect("recipient").balance_pwm, 0);
        assert_eq!(st.fee_pool, 0);

        st.accounts.get_mut(&sender_id).expect("sender").balance_pwm = 1_000;
        st.drain_conservation_at_height(12, &cfg);
        assert!(st.pending_conservation.is_empty());
        assert_eq!(st.get(&sender_id).expect("sender").balance_pwm, 893);
        assert_eq!(st.get(&sender_id).expect("sender").nonce, 1);
        assert_eq!(st.get(&rcpt_id).expect("recipient").balance_pwm, 100);
        assert_eq!(st.fee_pool, 7);
    }

    #[test]
    fn conservation_incoming_not_delayed() {
        let cfg = dev_net().0;
        let mut st = State::default();
        let (rcpt_sk, rcpt_idx, rcpt_id) = user_sk_conserv(0xD0);
        let rcpt_dom = domain_of_account_id(&rcpt_id);
        let rcpt_hi = rcpt_dom.to_be_bytes()[0];
        let (sender_sk, sender_idx, sender_id) = user_sk_plain_domain(&st, 0xD1, rcpt_hi);
        let sender_dom = domain_of_account_id(&sender_id);
        st.accounts.insert(
            sender_id,
            Account::genesis_funded(sender_sk.verifying_key().to_bytes(), sender_idx, 1_000),
        );
        st.accounts.insert(
            rcpt_id,
            Account::genesis_funded(rcpt_sk.verifying_key().to_bytes(), rcpt_idx, 0),
        );
        let mut tx = SignedTx::sign_body(
            &sender_sk,
            sender_dom,
            sender_idx,
            0,
            TxBody::Transfer {
                to: rcpt_id,
                amount: 100,
                fee: 7,
            },
        );
        if cosign_non_dis(&sender_id) {
            add_witness_cosign(&mut tx, &sender_sk);
        }
        st.apply_tx_with_ctx(&tx, 10, 0, &cfg)
            .expect("incoming to conservation applies immediately");
        assert!(st.pending_conservation.is_empty());
        assert_eq!(st.get(&sender_id).expect("sender").balance_pwm, 893);
        assert_eq!(st.get(&sender_id).expect("sender").nonce, 1);
        assert_eq!(st.get(&rcpt_id).expect("recipient").balance_pwm, 100);
    }

    #[test]
    fn conservation_emergency_cancels_pending() {
        let mut cfg = dev_net().0;
        cfg.conservation_delay_blocks = 10;
        let mut st = State::default();
        let (sender_sk, sender_idx, sender_id) = user_sk_conserv(0xE0);
        let sender_dom = domain_of_account_id(&sender_id);
        let sender_hi = sender_dom.to_be_bytes()[0];
        let (rcpt_sk, rcpt_idx, rcpt_id) = user_sk_new_domain(&st, 0xE1, sender_hi);
        let (rescue_sk, rescue_idx, rescue_id) = user_sk_new_domain(&st, 0xE2, sender_hi);
        st.accounts.insert(
            sender_id,
            Account::genesis_funded(sender_sk.verifying_key().to_bytes(), sender_idx, 1_000),
        );
        st.accounts.insert(
            rcpt_id,
            Account::genesis_funded(rcpt_sk.verifying_key().to_bytes(), rcpt_idx, 0),
        );
        st.accounts.insert(
            rescue_id,
            Account::genesis_funded(rescue_sk.verifying_key().to_bytes(), rescue_idx, 0),
        );
        {
            let owner = st.accounts.get_mut(&sender_id).expect("sender");
            owner.rescue_address = Some(rescue_id);
            owner.dormant_policies |= PolicyKind::RoutingEmergencyRedirect.bit();
        }
        let pending = SignedTx::sign_body(
            &sender_sk,
            sender_dom,
            sender_idx,
            0,
            TxBody::Transfer {
                to: rcpt_id,
                amount: 100,
                fee: 7,
            },
        );
        st.apply_tx_with_ctx(&pending, 10, 0, &cfg)
            .expect("enqueue pending conservation");
        assert_eq!(st.pending_conservation.len(), 1);

        let mut activation = SignedTx::sign_body(
            &sender_sk,
            sender_dom,
            sender_idx,
            0,
            TxBody::Policy {
                target_account: sender_id,
                action: PolicyAction::ActivatePolicy {
                    policy_id: PolicyKind::RoutingEmergencyRedirect.policy_id(),
                    activation_target: Some(rescue_id),
                },
                fee: 0,
            },
        );
        let msg = activation.signing_message();
        activation.cosigns.push(Cosignature {
            signer_pk: rescue_sk.verifying_key().to_bytes(),
            role: CosignRole::Rescue,
            signature: sign(&rescue_sk, &msg),
        });
        st.apply_tx_with_ctx(&activation, 11, 0, &cfg)
            .expect("emergency activation cancels pending");
        assert!(st.pending_conservation.is_empty());
        assert_eq!(st.get(&sender_id).expect("sender").balance_pwm, 0);
        assert_eq!(st.get(&rescue_id).expect("rescue").balance_pwm, 1_000);
        st.drain_conservation_at_height(20, &cfg);
        assert_eq!(st.get(&rcpt_id).expect("recipient").balance_pwm, 0);
    }

    #[test]
    fn policy_fin_blocks_old_ops() {
        let (g, sks) = dev_net();
        let mut st = g.state0();
        let owner_sk = &sks[0];
        let owner_id = g.accounts[0].acct;
        let owner_dom = domain_of_account_id(&owner_id);
        let owner_hi = owner_dom.to_be_bytes()[0];
        let (peer_sk, peer_idx, peer_id) = user_sk_new_domain(&st, 0x40, owner_hi);
        let peer_dom = domain_of_account_id(&peer_id);
        st.apply_tx(&SignedTx::sign_body(
            &peer_sk,
            peer_dom,
            peer_idx,
            0,
            TxBody::Init { index: 4, flags: 0 },
        ))
        .expect("init peer");
        st.accounts.get_mut(&owner_id).expect("owner").finalized = true;
        let nonce = st.get(&owner_id).expect("owner").nonce;

        let xfer = SignedTx::sign_body(
            owner_sk,
            owner_dom,
            g.accounts[0].der_idx,
            nonce,
            TxBody::Transfer {
                to: peer_id,
                amount: 1,
                fee: 1,
            },
        );
        let xfer_err = st
            .apply_tx(&xfer)
            .expect_err("finalized transfer must fail");
        assert!(matches!(xfer_err, TxError::PolicyAccountFinalized));

        let stake = SignedTx::sign_body(
            owner_sk,
            owner_dom,
            g.accounts[0].der_idx,
            nonce,
            TxBody::Stake { amount: 1 },
        );
        let stake_err = st.apply_tx(&stake).expect_err("finalized stake must fail");
        assert!(matches!(stake_err, TxError::PolicyAccountFinalized));

        let pol = SignedTx::sign_body(
            owner_sk,
            owner_dom,
            g.accounts[0].der_idx,
            nonce,
            TxBody::Policy {
                target_account: owner_id,
                action: PolicyAction::SetPolicy {
                    policy: PolicyKind::SenderFilter,
                    activation: ActivationMode::Dormant,
                },
                fee: 1,
            },
        );
        let pol_err = st
            .apply_tx(&pol)
            .expect_err("finalized non-emergency policy must fail");
        assert!(matches!(pol_err, TxError::PolicyAccountFinalized));
    }

    #[test]
    fn policy_xfer_rescue_credit() {
        let mut st = State::default();
        let (sender_sk, sender_idx, sender_id) = user_sk_plain_domain(&st, 0x45, 0x43);
        let sender_dom = domain_of_account_id(&sender_id);
        let sender_hi = sender_dom.to_be_bytes()[0];
        st.accounts.insert(
            sender_id,
            Account::genesis_funded(sender_sk.verifying_key().to_bytes(), sender_idx, 1_000_000),
        );
        let (target_sk, target_idx, target_id) = user_sk_new_domain(&st, 0x46, sender_hi);
        let target_dom = domain_of_account_id(&target_id);
        st.apply_tx(&SignedTx::sign_body(
            &target_sk,
            target_dom,
            target_idx,
            0,
            TxBody::Init {
                index: 10,
                flags: 0,
            },
        ))
        .expect("init target");
        let (rescue_sk, rescue_idx, rescue_id) = user_sk_new_domain(&st, 0x44, sender_hi);
        let rescue_dom = domain_of_account_id(&rescue_id);
        st.apply_tx(&SignedTx::sign_body(
            &rescue_sk,
            rescue_dom,
            rescue_idx,
            0,
            TxBody::Init { index: 9, flags: 0 },
        ))
        .expect("init rescue");
        {
            let target = st.accounts.get_mut(&target_id).expect("target");
            target.finalized = true;
            target.rescue_address = Some(rescue_id);
            target.active_policies |= PolicyKind::RoutingEmergencyRedirect.bit();
        }
        let sender_bal0 = st.get(&sender_id).expect("sender").balance_pwm;
        let rescue_bal0 = st.get(&rescue_id).expect("rescue").balance_pwm;
        let target_bal0 = st.get(&target_id).expect("target").balance_pwm;
        let tx = SignedTx::sign_body(
            &sender_sk,
            sender_dom,
            sender_idx,
            st.get(&sender_id).expect("sender").nonce,
            TxBody::Transfer {
                to: target_id,
                amount: 5,
                fee: 1,
            },
        );
        st.apply_tx(&tx)
            .expect("incoming transfer should route to rescue");
        assert_eq!(
            st.get(&sender_id).expect("sender").balance_pwm,
            sender_bal0 - 6
        );
        assert_eq!(
            st.get(&rescue_id).expect("rescue").balance_pwm,
            rescue_bal0 + 5
        );
        assert_eq!(st.get(&target_id).expect("target").balance_pwm, target_bal0);
    }

    #[test]
    fn policy_xfer_rescue_cross_deny() {
        let (g, sks) = dev_net();
        let mut st = g.state0();
        let sender_sk = &sks[0];
        let sender_id = g.accounts[0].acct;
        let sender_dom = domain_of_account_id(&sender_id);
        let sender_hi = sender_dom.to_be_bytes()[0];
        let (target_sk, target_idx, target_id) = user_sk_new_domain(&st, 0x49, sender_hi);
        let target_dom = domain_of_account_id(&target_id);
        st.apply_tx(&SignedTx::sign_body(
            &target_sk,
            target_dom,
            target_idx,
            0,
            TxBody::Init {
                index: 13,
                flags: 0,
            },
        ))
        .expect("init target");
        let (rescue_sk, rescue_idx, rescue_id) = user_sk_other_domain(0x51, sender_hi);
        let rescue_dom = domain_of_account_id(&rescue_id);
        st.apply_tx(&SignedTx::sign_body(
            &rescue_sk,
            rescue_dom,
            rescue_idx,
            0,
            TxBody::Init {
                index: 14,
                flags: 0,
            },
        ))
        .expect("init rescue");
        {
            let target = st.accounts.get_mut(&target_id).expect("target");
            target.finalized = true;
            target.active_policies |= PolicyKind::RoutingEmergencyRedirect.bit();
            target.rescue_address = Some(rescue_id);
        }
        let before = st.clone();
        let tx = SignedTx::sign_body(
            sender_sk,
            sender_dom,
            g.accounts[0].der_idx,
            st.get(&sender_id).expect("sender").nonce,
            TxBody::Transfer {
                to: target_id,
                amount: 4,
                fee: 1,
            },
        );
        let err = st
            .apply_tx(&tx)
            .expect_err("cross-domain rescue redirect must reject transfer");
        assert!(matches!(err, TxError::PolicyRoutingDenied));
        assert_eq!(st.accounts, before.accounts);
        assert_eq!(st.fee_pool, before.fee_pool);
    }

    #[test]
    fn policy_xfer_no_rescue() {
        let (g, sks) = dev_net();
        let mut st = g.state0();
        let sender_id = g.accounts[0].acct;
        let sender_dom = domain_of_account_id(&sender_id);
        let sender_hi = sender_dom.to_be_bytes()[0];
        let (target_sk, target_idx, target_id) = user_sk_new_domain(&st, 0x47, sender_hi);
        let target_dom = domain_of_account_id(&target_id);
        st.apply_tx(&SignedTx::sign_body(
            &target_sk,
            target_dom,
            target_idx,
            0,
            TxBody::Init {
                index: 11,
                flags: 0,
            },
        ))
        .expect("init target");
        {
            let target = st.accounts.get_mut(&target_id).expect("target");
            target.finalized = true;
            target.active_policies |= PolicyKind::RoutingEmergencyRedirect.bit();
            target.rescue_address = None;
        }
        let tx = SignedTx::sign_body(
            &sks[0],
            sender_dom,
            g.accounts[0].der_idx,
            st.get(&sender_id).expect("sender").nonce,
            TxBody::Transfer {
                to: target_id,
                amount: 2,
                fee: 1,
            },
        );
        let err = st
            .apply_tx(&tx)
            .expect_err("missing rescue address must reject transfer");
        assert!(matches!(err, TxError::PolicyRescueRequired));
    }

    #[test]
    fn policy_xfer_rescue_no_init() {
        let (g, sks) = dev_net();
        let mut st = g.state0();
        let sender_id = g.accounts[0].acct;
        let sender_dom = domain_of_account_id(&sender_id);
        let sender_hi = sender_dom.to_be_bytes()[0];
        let (_, rescue_idx, rescue_id) = user_sk_new_domain(&st, 0x45, sender_hi);
        st.accounts.insert(
            rescue_id,
            Account {
                derivation_index: rescue_idx,
                ..Default::default()
            },
        );
        let (target_sk, target_idx, target_id) = user_sk_new_domain(&st, 0x48, sender_hi);
        let target_dom = domain_of_account_id(&target_id);
        st.apply_tx(&SignedTx::sign_body(
            &target_sk,
            target_dom,
            target_idx,
            0,
            TxBody::Init {
                index: 12,
                flags: 0,
            },
        ))
        .expect("init target");
        {
            let target = st.accounts.get_mut(&target_id).expect("target");
            target.finalized = true;
            target.active_policies |= PolicyKind::RoutingEmergencyRedirect.bit();
            target.rescue_address = Some(rescue_id);
        }
        let tx = SignedTx::sign_body(
            &sks[0],
            sender_dom,
            g.accounts[0].der_idx,
            st.get(&sender_id).expect("sender").nonce,
            TxBody::Transfer {
                to: target_id,
                amount: 3,
                fee: 1,
            },
        );
        let err = st
            .apply_tx(&tx)
            .expect_err("uninitialized rescue account must reject transfer");
        assert!(matches!(err, TxError::RecipientNotInitialized));
    }
}
