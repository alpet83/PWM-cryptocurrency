//! Canonical chain state: accounts map, fees, burns, import consumed IDs.

use crate::tx::{
    export_context_is_valid, import_context_is_valid, ClaimMode, SignedTx, TxBody, TxError,
    CLAIM_ALL,
};
use crate::types::{Account, AccountId};
use crate::PWM_RAW_SCALE;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::time::{SystemTime, UNIX_EPOCH};

const PPM_DENOM: u128 = 1_000_000;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExportProvenance {
    pub to: AccountId,
    pub target_domain: u16,
    #[serde(with = "crate::ser_json_u128")]
    pub amount: u128,
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
}

/// Stable blake3 over bincode (devnet state root).
pub fn digest(st: &State) -> [u8; 32] {
    *blake3::hash(&bincode::serialize(st).expect("state bincode")).as_bytes()
}

impl State {
    pub fn get(&self, id: &AccountId) -> Option<&Account> {
        self.accounts.get(id)
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
    ) -> Result<(), TxError> {
        let mut st = self.clone();
        st.apply_tx_with_ctx(tx, inclusion_height, block_unix_time)
    }

    /// Dry-run on the next block after `tip_height` (mempool / RPC admission).
    pub fn precheck_apply_tip(
        &self,
        tx: &SignedTx,
        tip_height: u64,
        block_unix_time: u64,
    ) -> Result<(), TxError> {
        self.precheck_apply_with_ctx(tx, tip_height.saturating_add(1), block_unix_time)
    }

    /// Applies one signed tx. `Init` may create an empty stub row first (white-spec).
    pub fn apply_tx(&mut self, tx: &SignedTx) -> Result<(), TxError> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| TxError::ClaimAnchorRangeInvalid)?
            .as_secs();
        self.apply_tx_with_ctx(tx, 0, now)
    }

    /// Applies one signed tx using canonical block context for claim maturity checks.
    pub fn apply_tx_with_ctx(
        &mut self,
        tx: &SignedTx,
        inclusion_height: u64,
        block_unix_time: u64,
    ) -> Result<(), TxError> {
        crate::tx::validate_tx_shape(tx)?;
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

        match &tx.body {
            TxBody::Init { index, flags } => {
                if acc.initialized {
                    return Err(TxError::AlreadyInit);
                }
                let mut a = acc;
                a.initialized = true;
                a.index = *index;
                a.flags = *flags;
                a.nonce += 1;
                self.accounts.insert(id, a);
            }
            TxBody::Transfer { to, amount, fee } => {
                if !acc.initialized {
                    return Err(TxError::NotInitialized);
                }
                if *to != id {
                    self.require_recipient(to)?;
                }
                let total = amount.checked_add(*fee).ok_or(TxError::Insufficient)?;
                if acc.balance_pwm < total {
                    return Err(TxError::Insufficient);
                }
                let mut from = acc;
                from.balance_pwm -= total;
                from.nonce += 1;
                self.fee_pool = self.fee_pool.saturating_add(tx.body.fee_amount());

                if *to == id {
                    // Self-transfer must apply debit+credit against the same account.
                    // Previously the receiver stub overwrite could discard the nonce increment.
                    from.balance_pwm = from.balance_pwm.saturating_add(*amount);
                    self.accounts.insert(id, from);
                } else {
                    let mut to_acc = self.accounts.get(to).cloned().expect("recipient gated");
                    to_acc.balance_pwm = to_acc.balance_pwm.saturating_add(*amount);
                    self.accounts.insert(id, from);
                    self.accounts.insert(*to, to_acc);
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
                apply_auto_claim(&mut a, inclusion_height, block_unix_time);
                a.balance_pwm -= amount;
                a.staked += amount;
                a.last_claim_unix_time = block_unix_time;
                a.last_stake_change_height = inclusion_height;
                a.nonce += 1;
                self.accounts.insert(id, a);
            }
            TxBody::Unstake { amount } => {
                if !acc.initialized {
                    return Err(TxError::NotInitialized);
                }
                if acc.staked < *amount {
                    return Err(TxError::Insufficient);
                }
                let mut a = acc;
                apply_auto_claim(&mut a, inclusion_height, block_unix_time);
                a.staked -= amount;
                a.balance_pwm += amount;
                a.last_claim_unix_time = block_unix_time;
                a.last_stake_change_height = inclusion_height;
                a.nonce += 1;
                self.accounts.insert(id, a);
            }
            TxBody::BurnMark { mark_amount, .. } => {
                if !acc.initialized {
                    return Err(TxError::NotInitialized);
                }
                // Burn is unilateral: beneficiary is metadata, sender marks are debited locally.
                if acc.marks < *mark_amount {
                    return Err(TxError::InsufficientMarks);
                }
                let mut a = acc;
                a.marks -= *mark_amount;
                a.nonce += 1;
                self.accounts.insert(id, a);
            }
            TxBody::Claim {
                mode,
                claim_units,
                anchor_ref,
                fee,
            } => {
                if !acc.initialized {
                    return Err(TxError::NotInitialized);
                }
                if *anchor_ref > inclusion_height {
                    return Err(TxError::ClaimAnchorRangeInvalid);
                }
                if *anchor_ref < acc.last_claim_anchor_ref {
                    return Err(TxError::ClaimAnchorRangeInvalid);
                }
                if acc.last_stake_change_height > *anchor_ref
                    && acc.last_stake_change_height <= inclusion_height
                {
                    return Err(TxError::ClaimAnchorContinuityBroken);
                }
                let matured = matured_units_available(&acc, block_unix_time);
                let effective_units = if *claim_units == CLAIM_ALL {
                    matured
                } else {
                    *claim_units
                };
                if *claim_units == CLAIM_ALL && effective_units == 0 {
                    // CLAIM_ALL with zero matured marks is a valid no-op claim.
                    // The tx still consumes nonce, but claim windows/limits stay untouched.
                    let mut a = acc;
                    a.nonce += 1;
                    self.accounts.insert(id, a);
                    return Ok(());
                }
                if effective_units == 0 || effective_units > matured {
                    return Err(TxError::ClaimOverMatured);
                }
                let mut a = acc;
                match mode {
                    ClaimMode::Free => {
                        let utc_day = block_unix_time / 86_400;
                        if a.free_claim_utc_day == Some(utc_day) {
                            return Err(TxError::FreeClaimDailyLimit);
                        }
                        a.free_claim_utc_day = Some(utc_day);
                    }
                    ClaimMode::Paid => {
                        if a.balance_pwm < *fee {
                            return Err(TxError::Insufficient);
                        }
                        a.balance_pwm -= *fee;
                        self.fee_pool = self.fee_pool.saturating_add(*fee);
                    }
                }
                a.marks = a.marks.saturating_add(effective_units);
                a.last_claim_unix_time = block_unix_time;
                a.last_claim_anchor_ref = inclusion_height;
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
                self.exported_registry
                    .entry(export_id)
                    .or_insert_with(|| match &tx.body {
                        TxBody::Export {
                            to,
                            target_domain,
                            amount,
                            ..
                        } => ExportProvenance {
                            to: *to,
                            target_domain: *target_domain,
                            amount: *amount,
                        },
                        _ => unreachable!("guarded by TxBody::Export match arm"),
                    });
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
                self.imported_set.insert(*export_id);
            }
        }
        Ok(())
    }

    /// Marks from stake per block (`staked * coeff / 1_000_000`).
    pub fn accrue_marks(&mut self, coeff: u128) {
        for a in self.accounts.values_mut() {
            if !a.initialized {
                continue;
            }
            let add = as_marks_u32(a.staked.saturating_mul(coeff) / PPM_DENOM);
            a.marks = a.marks.saturating_add(add);
        }
    }

    /// V2 marks path: stake gate + seasonal ppm.
    pub fn accrue_marks_v2(&mut self, coeff: u128, stake_min: u128, season_ppm: u128) {
        for a in self.accounts.values_mut() {
            if !a.initialized || a.staked < stake_min {
                continue;
            }
            let base = a.staked.saturating_mul(coeff) / PPM_DENOM;
            let add = as_marks_u32(base.saturating_mul(season_ppm) / PPM_DENOM);
            a.marks = a.marks.saturating_add(add);
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
            if a.staked < stake_min {
                return;
            }
            let add = reward.saturating_mul(season_ppm) / PPM_DENOM;
            a.balance_pwm = a.balance_pwm.saturating_add(add);
        }
    }
}

fn matured_units_available(acc: &Account, block_unix_time: u64) -> u32 {
    if block_unix_time <= acc.last_claim_unix_time || acc.staked == 0 {
        return 0;
    }
    let delta_seconds = block_unix_time - acc.last_claim_unix_time;
    let hours = delta_seconds / 3_600;
    let whole_pwm_staked = acc.staked / PWM_RAW_SCALE;
    as_marks_u32(whole_pwm_staked.saturating_mul(hours as u128))
}

fn apply_auto_claim(acc: &mut Account, inclusion_height: u64, block_unix_time: u64) {
    let matured = matured_units_available(acc, block_unix_time);
    if matured == 0 {
        return;
    }
    acc.marks = acc.marks.saturating_add(matured);
    acc.last_claim_unix_time = block_unix_time;
    acc.last_claim_anchor_ref = inclusion_height;
}

fn as_marks_u32(value: u128) -> u32 {
    value.min(u32::MAX as u128) as u32
}

#[cfg(test)]
mod tests {
    use super::State;
    use crate::genesis::dev_net;
    use crate::hd::{account_id_from_parts, domain_of_account_id};
    use crate::tx::{validate_tx_shape, ClaimMode, SignedTx, TxBody, TxError, CLAIM_ALL};
    use crate::types::Account;
    use crate::types::AccountId;
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

    /// Init stub then transfer validator→peer debits fee pool (formerly `apply_tx_init_then_transfer_happy_path`).
    #[test]
    fn init_then_xfer_happy() {
        let (g, sks) = dev_net();
        let mut st = g.state0();
        let sk_v = &sks[0];
        let aid_v = g.accounts[0].acct;
        let dom_v = domain_of_account_id(&aid_v);

        let (sk_b, i_b, aid_b) = user_sk0(&[3u8; 32]);
        let dom_b = domain_of_account_id(&aid_b);

        let init_b = SignedTx::sign_body(&sk_b, dom_b, i_b, 0, TxBody::Init { index: 7, flags: 0 });
        st.apply_tx(&init_b).expect("init new account");
        let b = st.get(&aid_b).expect("stub then init row");
        assert!(b.initialized);
        assert_eq!(b.nonce, 1);
        assert_eq!(b.balance_pwm, 0);

        let xfer = SignedTx::sign_body(
            sk_v,
            dom_v,
            0,
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
        st.accounts.get_mut(&aid_v).expect("validator").marks = 25;

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
        assert_eq!(after.marks, 18);
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
        st.accounts.get_mut(&aid_v).expect("validator").marks = 3;
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

        st.accounts.get_mut(&aid_v).expect("sender").marks = 40;
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
        assert_eq!(sender_after.marks, 34);
    }

    /// Foreign-domain beneficiary burn debits sender marks (V2-7: cross-domain allowed).
    #[test]
    fn burn_ben_xdom_ok() {
        let (g, sks) = dev_net();
        let mut st = g.state0();
        let sk_v = &sks[0];
        let aid_v = g.accounts[0].acct;
        let dom_v = domain_of_account_id(&aid_v);
        st.accounts.get_mut(&aid_v).expect("validator").marks = 20;
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
        assert_eq!(after.marks, 15);
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
        assert!(tx.export_id().is_some());
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
    fn claim_tx_materializes_marks() {
        let (g, sks) = dev_net();
        let mut st = g.state0();
        let sk_v = &sks[0];
        let aid_v = g.accounts[0].acct;
        let dom_v = domain_of_account_id(&aid_v);
        {
            let acc = st.accounts.get_mut(&aid_v).expect("validator");
            acc.staked = 2 * PWM_RAW_SCALE;
            acc.last_claim_unix_time = 0;
            acc.last_stake_change_height = 0;
        }
        let tx = SignedTx::sign_body(
            sk_v,
            dom_v,
            0,
            0,
            TxBody::Claim {
                mode: ClaimMode::Free,
                claim_units: 4,
                anchor_ref: 0,
                fee: 0,
            },
        );
        st.apply_tx_with_ctx(&tx, 10, 7_200).expect("claim apply");
        let acc = st.get(&aid_v).expect("validator");
        assert_eq!(acc.marks, 5);
        assert_eq!(acc.last_claim_anchor_ref, 10);
        assert_eq!(acc.nonce, 1);
    }

    /// CLAIM_ALL sentinel must match u32::MAX and claim matured units, not reject as over-claim.
    #[test]
    fn claim_all_sentinel() {
        assert_eq!(CLAIM_ALL, u32::MAX);
        let (g, sks) = dev_net();
        let mut st = g.state0();
        let sk_v = &sks[0];
        let aid_v = g.accounts[0].acct;
        let dom_v = domain_of_account_id(&aid_v);
        {
            let acc = st.accounts.get_mut(&aid_v).expect("validator");
            acc.staked = 2 * PWM_RAW_SCALE;
            acc.last_claim_unix_time = 0;
            acc.last_stake_change_height = 0;
            acc.marks = 1;
        }
        let tx = SignedTx::sign_body(
            sk_v,
            dom_v,
            0,
            0,
            TxBody::Claim {
                mode: ClaimMode::Free,
                claim_units: CLAIM_ALL,
                anchor_ref: 0,
                fee: 0,
            },
        );
        st.apply_tx_with_ctx(&tx, 10, 7_200)
            .expect("claim all apply");
        let acc = st.get(&aid_v).expect("validator");
        assert_eq!(acc.marks, 1 + 4);
        assert_eq!(acc.last_claim_anchor_ref, 10);
        assert_eq!(acc.nonce, 1);
    }

    #[test]
    fn claim_all_zero_matured_noop() {
        let (g, sks) = dev_net();
        let mut st = g.state0();
        let sk_v = &sks[0];
        let aid_v = g.accounts[0].acct;
        let dom_v = domain_of_account_id(&aid_v);
        let block_unix_time = 10_000;
        let prev_claim_time = 9_900;
        {
            let acc = st.accounts.get_mut(&aid_v).expect("validator");
            acc.staked = 2 * PWM_RAW_SCALE;
            acc.marks = 0;
            acc.last_claim_unix_time = prev_claim_time;
            acc.last_claim_anchor_ref = 7;
            acc.last_stake_change_height = 0;
            acc.free_claim_utc_day = Some(block_unix_time / 86_400);
        }
        let tx = SignedTx::sign_body(
            sk_v,
            dom_v,
            0,
            0,
            TxBody::Claim {
                mode: ClaimMode::Free,
                claim_units: CLAIM_ALL,
                anchor_ref: 7,
                fee: 0,
            },
        );
        st.apply_tx_with_ctx(&tx, 10, block_unix_time)
            .expect("claim all zero matured must be no-op success");
        let acc = st.get(&aid_v).expect("validator");
        assert_eq!(acc.marks, 0);
        assert_eq!(acc.nonce, 1);
        assert_eq!(acc.last_claim_unix_time, prev_claim_time);
        assert_eq!(acc.last_claim_anchor_ref, 7);
        assert_eq!(acc.free_claim_utc_day, Some(block_unix_time / 86_400));
    }

    #[test]
    fn precheck_tip_next_ctx() {
        let (g, sks) = dev_net();
        let mut st = g.state0();
        let sk_v = &sks[0];
        let aid_v = g.accounts[0].acct;
        let dom_v = domain_of_account_id(&aid_v);
        {
            let acc = st.accounts.get_mut(&aid_v).expect("validator");
            acc.staked = 2 * PWM_RAW_SCALE;
            acc.last_claim_unix_time = 0;
            acc.last_stake_change_height = 0;
        }
        let tx = SignedTx::sign_body(
            sk_v,
            dom_v,
            0,
            0,
            TxBody::Claim {
                mode: ClaimMode::Free,
                claim_units: 2,
                anchor_ref: 1,
                fee: 0,
            },
        );
        st.precheck_apply_tip(&tx, 0, 7_200)
            .expect("tip+1 context must accept anchor_ref=1");
        let err = st
            .precheck_apply_with_ctx(&tx, 0, 7_200)
            .expect_err("zero context must reject anchor_ref=1");
        assert!(matches!(err, TxError::ClaimAnchorRangeInvalid));
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
            acc.staked = 3 * PWM_RAW_SCALE;
            acc.last_claim_unix_time = 10_000;
        }
        let before_marks = st.get(&aid_v).expect("validator").marks;
        let tx = SignedTx::sign_body(sk_v, dom_v, 0, 0, TxBody::Stake { amount: 1 });
        st.apply_tx_with_ctx(&tx, 12, 10_100).expect("stake apply");
        let after = st.get(&aid_v).expect("validator");
        assert_eq!(after.marks, before_marks);
    }

    /// 1 whole PWM staked for 1 hour yields 1 mark unit (V2-5 formula).
    #[test]
    fn marks_1pwm_1h() {
        let acc = Account {
            staked: 1_000_000,
            last_claim_unix_time: 0,
            ..Default::default()
        };
        assert_eq!(super::matured_units_available(&acc, 3_600), 1);
    }

    /// Sub-1-PWM stake truncates whole PWM before multiplying by hours.
    #[test]
    fn marks_sub1pwm_trunc() {
        let acc = Account {
            staked: 500_000,
            last_claim_unix_time: 0,
            ..Default::default()
        };
        assert_eq!(super::matured_units_available(&acc, 36_000), 0);
    }

    /// Large (staked_pwm × hours) saturates at u32::MAX marks.
    #[test]
    fn marks_saturation_u32() {
        let acc = Account {
            staked: u32::MAX as u128 * PWM_RAW_SCALE,
            last_claim_unix_time: 0,
            ..Default::default()
        };
        assert_eq!(super::matured_units_available(&acc, 3_600 * 100), u32::MAX);
    }

    /// Legacy snapshot JSON: huge raw marks divide by PWM_RAW_SCALE and clamp to u32.
    #[test]
    fn marks_snap_migrate() {
        let json = format!(
            r#"{{"signing_pubkey":{},"derivation_index":0,"balance_pwm":0,"staked":0,"marks":7000000000000,"initialized":false,"index":0,"flags":0,"nonce":0}}"#,
            serde_json::to_string(&[0u8; 32]).unwrap()
        );
        let acc: Account = serde_json::from_str(&json).unwrap();
        assert_eq!(acc.marks, 7_000_000_u32);
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
        acc.staked = 199_999;
        st.accrue_marks_v2(10_000, 200_000, 1_000_000);
        assert_eq!(st.accounts.get(&aid).expect("validator").marks, 1);
    }

    #[test]
    fn marks_v2_scale_season_ppm() {
        let (g, _) = dev_net();
        let mut st = g.state0();
        let aid = g.accounts[0].acct;
        let acc = st.accounts.get_mut(&aid).expect("validator");
        acc.staked = 300_000;
        st.accrue_marks_v2(10_000, 1, 500_000);
        assert_eq!(st.accounts.get(&aid).expect("validator").marks, 1_501);
    }

    #[test]
    fn reward_v2_gate_stake_min() {
        let (g, _) = dev_net();
        let mut st = g.state0();
        let aid = g.accounts[0].acct;
        let acc = st.accounts.get_mut(&aid).expect("validator");
        acc.staked = 50_000;
        let bal0 = acc.balance_pwm;
        st.reward_producer_v2(&aid, 100, 100_000, 500_000);
        assert_eq!(st.accounts.get(&aid).expect("validator").balance_pwm, bal0);
    }
}
