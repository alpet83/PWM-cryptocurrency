//! Signed transaction bodies, validation, and hashing helpers shared by crates.

use crate::crypto::{blake3_32, sign, verify};
use crate::hd::{account_id_from_parts, domain_of_account_id};
use crate::state::ExportProvenance;
use crate::types::AccountId;
use ed25519_dalek::SigningKey;
use serde::{Deserialize, Serialize};

pub const MIN_IMPORT_FEE_UNITS: u128 = 10_000;
/// Sentinel value for `TxBody::Claim.claim_units`: instructs the node to
/// materialise all currently matured marks without the client computing the amount.
pub const CLAIM_ALL: u32 = u32::MAX;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ClaimMode {
    Free,
    Paid,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TxBody {
    Init {
        index: u32,
        flags: u32,
    },
    Transfer {
        to: AccountId,
        #[serde(with = "crate::ser_json_u128")]
        amount: u128,
        #[serde(with = "crate::ser_json_u128")]
        fee: u128,
    },
    Stake {
        #[serde(with = "crate::ser_json_u128")]
        amount: u128,
    },
    Unstake {
        #[serde(with = "crate::ser_json_u128")]
        amount: u128,
    },
    BurnMark {
        mark_amount: u32,
        beneficiary: Option<AccountId>,
    },
    Claim {
        mode: ClaimMode,
        claim_units: u32,
        anchor_ref: u64,
        #[serde(with = "crate::ser_json_u128")]
        fee: u128,
    },
    Export {
        to: AccountId,
        target_domain: u16,
        #[serde(with = "crate::ser_json_u128")]
        amount: u128,
        #[serde(with = "crate::ser_json_u128")]
        fee: u128,
    },
    Import {
        to: AccountId,
        #[serde(with = "crate::ser_json_u128")]
        amount: u128,
        export_id: [u8; 32],
    },
}

impl TxBody {
    /// Canonical fee view used by policy checks and state invariants.
    /// Burn-mark flow is fixed to zero fee in Sprint 8 baseline.
    pub fn fee_amount(&self) -> u128 {
        match self {
            TxBody::Transfer { fee, .. }
            | TxBody::Export { fee, .. }
            | TxBody::Claim { fee, .. } => *fee,
            TxBody::Init { .. }
            | TxBody::Stake { .. }
            | TxBody::Unstake { .. }
            | TxBody::BurnMark { .. }
            | TxBody::Import { .. } => 0,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SignedTx {
    pub domain_code: u16,
    pub signer_pk: [u8; 32],
    pub derivation_index: u32,
    pub nonce: u64,
    pub body: TxBody,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub burn_purpose: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "crate::ser_json_u128::opt")]
    pub import_fee: Option<u128>,
    /// Optional embedded provenance for target-side IMPORT replay determinism.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub import_provenance: Option<ExportProvenance>,
    /// Ed25519 sig, 64 bytes.
    #[serde(with = "crate::ser_bin::sig64")]
    pub signature: [u8; 64],
}

impl SignedTx {
    pub const EXPORT_OUTPUT_INDEX: u32 = 0;

    pub fn computed_account_id(&self) -> AccountId {
        account_id_from_parts(&self.signer_pk, self.derivation_index)
    }

    pub fn signing_message(&self) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(b"PWMv0/TX");
        v.extend_from_slice(&self.domain_code.to_le_bytes());
        v.extend_from_slice(&self.signer_pk);
        v.extend_from_slice(&self.derivation_index.to_le_bytes());
        v.extend_from_slice(&self.nonce.to_le_bytes());
        match &self.body {
            TxBody::Init { index, flags } => {
                v.push(1);
                v.extend_from_slice(&index.to_le_bytes());
                v.extend_from_slice(&flags.to_le_bytes());
            }
            TxBody::Transfer { to, amount, fee } => {
                v.push(2);
                v.extend_from_slice(to);
                v.extend_from_slice(&amount.to_le_bytes());
                v.extend_from_slice(&fee.to_le_bytes());
            }
            TxBody::Stake { amount } => {
                v.push(3);
                v.extend_from_slice(&amount.to_le_bytes());
            }
            TxBody::Unstake { amount } => {
                v.push(4);
                v.extend_from_slice(&amount.to_le_bytes());
            }
            TxBody::BurnMark {
                mark_amount,
                beneficiary,
            } => {
                v.push(5);
                v.extend_from_slice(&mark_amount.to_le_bytes());
                let purpose = self
                    .burn_purpose
                    .as_deref()
                    .map(normalize_burn_purpose)
                    .unwrap_or_default();
                let purpose_bytes = purpose.as_bytes();
                let len_u16 = u16::try_from(purpose_bytes.len()).unwrap_or(u16::MAX);
                v.extend_from_slice(&len_u16.to_le_bytes());
                v.extend_from_slice(purpose_bytes);
                match beneficiary {
                    Some(b) => {
                        v.push(1);
                        v.extend_from_slice(b);
                    }
                    None => v.push(0),
                }
            }
            TxBody::Claim {
                mode,
                claim_units,
                anchor_ref,
                fee,
            } => {
                v.push(8);
                v.push(match mode {
                    ClaimMode::Free => 0,
                    ClaimMode::Paid => 1,
                });
                v.extend_from_slice(&claim_units.to_le_bytes());
                v.extend_from_slice(&anchor_ref.to_le_bytes());
                v.extend_from_slice(&fee.to_le_bytes());
            }
            TxBody::Export {
                to,
                target_domain,
                amount,
                fee,
            } => {
                v.push(6);
                v.extend_from_slice(to);
                v.extend_from_slice(&target_domain.to_le_bytes());
                v.extend_from_slice(&amount.to_le_bytes());
                v.extend_from_slice(&fee.to_le_bytes());
            }
            TxBody::Import {
                to,
                amount,
                export_id,
            } => {
                v.push(7);
                v.extend_from_slice(to);
                v.extend_from_slice(&amount.to_le_bytes());
                v.extend_from_slice(&self.import_fee.unwrap_or(0).to_le_bytes());
                v.extend_from_slice(export_id);
                match &self.import_provenance {
                    Some(p) => {
                        v.push(1);
                        v.extend_from_slice(&p.to);
                        v.extend_from_slice(&p.target_domain.to_le_bytes());
                        v.extend_from_slice(&p.amount.to_le_bytes());
                    }
                    None => v.push(0),
                }
            }
        }
        v
    }

    pub fn tx_hash(&self) -> [u8; 32] {
        blake3_32(&self.signing_message())
    }

    pub fn verify_sig(&self) -> bool {
        let msg = self.signing_message();
        verify(&self.signer_pk, &msg, &self.signature)
    }

    /// Deterministic identifier for source-side export commit:
    /// hash(source_domain || tx_hash || output_index || nonce).
    pub fn export_id(&self) -> Option<[u8; 32]> {
        let TxBody::Export { .. } = &self.body else {
            return None;
        };
        let mut v = Vec::new();
        v.extend_from_slice(&self.domain_code.to_le_bytes());
        v.extend_from_slice(&self.tx_hash());
        v.extend_from_slice(&Self::EXPORT_OUTPUT_INDEX.to_le_bytes());
        v.extend_from_slice(&self.nonce.to_le_bytes());
        Some(blake3_32(&v))
    }

    pub fn sign_body(
        signing_key: &SigningKey,
        domain_code: u16,
        derivation_index: u32,
        nonce: u64,
        body: TxBody,
    ) -> Self {
        let signer_pk = signing_key.verifying_key().to_bytes();
        let mut tx = Self {
            domain_code,
            signer_pk,
            derivation_index,
            nonce,
            body,
            burn_purpose: None,
            import_fee: None,
            import_provenance: None,
            signature: [0u8; 64],
        };
        match &tx.body {
            TxBody::BurnMark { .. } => tx.burn_purpose = Some("default".to_string()),
            TxBody::Import { .. } => tx.import_fee = Some(MIN_IMPORT_FEE_UNITS),
            _ => {}
        }
        let msg = tx.signing_message();
        tx.signature = sign(signing_key, &msg);
        tx
    }

    pub fn set_import_provenance_signed(
        &mut self,
        signing_key: &SigningKey,
        provenance: Option<ExportProvenance>,
    ) {
        self.import_provenance = provenance;
        let msg = self.signing_message();
        self.signature = sign(signing_key, &msg);
    }

    pub fn set_burn_purpose_signed(&mut self, signing_key: &SigningKey, purpose: String) {
        self.burn_purpose = Some(purpose);
        let msg = self.signing_message();
        self.signature = sign(signing_key, &msg);
    }

    pub fn set_import_fee_signed(&mut self, signing_key: &SigningKey, fee: u128) {
        self.import_fee = Some(fee);
        let msg = self.signing_message();
        self.signature = sign(signing_key, &msg);
    }
}

/// Structural checks before state application.
pub fn validate_tx_shape(tx: &SignedTx) -> Result<(), TxError> {
    if !tx.verify_sig() {
        return Err(TxError::BadSignature);
    }
    let aid = tx.computed_account_id();
    if domain_of_account_id(&aid) != tx.domain_code {
        return Err(TxError::DomainMismatch);
    }
    if let TxBody::Transfer { to, .. } = &tx.body {
        if *to == aid {
            return Err(TxError::InvalidTransfer);
        }
    }
    match &tx.body {
        TxBody::BurnMark { .. } => {
            let normalized = tx
                .burn_purpose
                .as_deref()
                .map(normalize_burn_purpose)
                .ok_or(TxError::InvalidPurposeLength)?;
            validate_burn_purpose(&normalized)?;
        }
        TxBody::Claim {
            mode,
            claim_units,
            fee,
            ..
        } => {
            if *claim_units == 0 {
                return Err(TxError::ClaimDeltaInvalid);
            }
            match mode {
                ClaimMode::Free if *fee != 0 => return Err(TxError::ClaimFeeModeConflict),
                ClaimMode::Paid if *fee == 0 => return Err(TxError::ClaimFeeModeConflict),
                _ => {}
            }
        }
        TxBody::Import { .. } => {
            if tx.import_fee.unwrap_or(0) < MIN_IMPORT_FEE_UNITS {
                return Err(TxError::ImportFeeTooLow);
            }
        }
        _ => {}
    }
    Ok(())
}

pub fn normalize_burn_purpose(value: &str) -> String {
    value.trim().to_string()
}

fn validate_burn_purpose(value: &str) -> Result<(), TxError> {
    let len = value.as_bytes().len();
    if !(1..=80).contains(&len) {
        return Err(TxError::InvalidPurposeLength);
    }
    if value
        .chars()
        .any(|ch| matches!(ch as u32, 0x0000..=0x001F | 0x007F..=0x009F))
    {
        return Err(TxError::InvalidPurposeChars);
    }
    Ok(())
}

pub fn same_hi_domain(from: &AccountId, to: &AccountId) -> bool {
    domain_of_account_id(from).to_be_bytes()[0] == domain_of_account_id(to).to_be_bytes()[0]
}

/// Source-side export context checks: explicit cross-domain route and recipient domain coherence.
pub fn export_context_is_valid(tx: &SignedTx) -> bool {
    match &tx.body {
        TxBody::Export {
            to,
            target_domain,
            amount,
            ..
        } => {
            if *amount == 0 {
                return false;
            }
            let src_hi = tx.domain_code.to_be_bytes()[0];
            let dst_hi = target_domain.to_be_bytes()[0];
            if src_hi == dst_hi {
                return false;
            }
            domain_of_account_id(to).to_be_bytes()[0] == dst_hi
        }
        _ => true,
    }
}

/// Target-side import context checks.
pub fn import_context_is_valid(tx: &SignedTx) -> bool {
    match &tx.body {
        TxBody::Import { to, amount, .. } => {
            if *amount == 0 {
                return false;
            }
            domain_of_account_id(to).to_be_bytes()[0] == tx.domain_code.to_be_bytes()[0]
        }
        _ => true,
    }
}

#[derive(Debug, thiserror::Error)]
pub enum TxError {
    #[error("bad signature")]
    BadSignature,
    #[error("domain mismatch")]
    DomainMismatch,
    #[error("account not found")]
    NoAccount,
    #[error("account not initialized")]
    NotInitialized,
    #[error("recipient account not found")]
    RecipientMissing,
    #[error("recipient account not initialized")]
    RecipientNotInitialized,
    #[error("bad nonce")]
    BadNonce,
    #[error("insufficient balance")]
    Insufficient,
    #[error("insufficient marks")]
    InsufficientMarks,
    #[error("already initialized")]
    AlreadyInit,
    #[error("invalid transfer")]
    InvalidTransfer,
    #[error("invalid export")]
    InvalidExport,
    #[error("invalid import")]
    InvalidImport,
    #[error("duplicate import")]
    DuplicateImport,
    #[error("invalid purpose length")]
    InvalidPurposeLength,
    #[error("invalid purpose chars")]
    InvalidPurposeChars,
    #[error("claim fee mode conflict")]
    ClaimFeeModeConflict,
    #[error("claim delta invalid")]
    ClaimDeltaInvalid,
    #[error("claim anchor range invalid")]
    ClaimAnchorRangeInvalid,
    #[error("claim anchor continuity broken")]
    ClaimAnchorContinuityBroken,
    #[error("claim over matured")]
    ClaimOverMatured,
    #[error("free claim daily limit")]
    FreeClaimDailyLimit,
    #[error("import fee too low")]
    ImportFeeTooLow,
}

#[cfg(test)]
mod tests {
    use super::{
        export_context_is_valid, import_context_is_valid, same_hi_domain, validate_tx_shape,
        SignedTx, TxBody, TxError, MIN_IMPORT_FEE_UNITS,
    };
    use crate::hd::domain_of_account_id;
    use ed25519_dalek::SigningKey;
    use serde_json::{from_slice, to_vec};
    use slip10_ed25519::derive_ed25519_private_key;

    fn signer(seed: &[u8; 32]) -> (SigningKey, u32) {
        let sk_bytes = derive_ed25519_private_key(seed, &[0, 0]);
        (SigningKey::from_bytes(&sk_bytes), 0)
    }

    /// Regulatory domain `0x2C00` rejects init at wrong lo byte (formerly `validate_tx_shape_accepts_regulatory_init_lo_zero`).
    #[test]
    fn reg_init_lo0_bad_shape() {
        let (sk, idx) = signer(&[31u8; 32]);
        let tx = SignedTx::sign_body(&sk, 0x2C00, idx, 0, TxBody::Init { index: 0, flags: 0 });
        let err = validate_tx_shape(&tx).expect_err("domain mismatch is expected in this fixture");
        assert!(matches!(err, TxError::DomainMismatch));
    }

    /// Regulatory `0x2C01` rejects init when signer lo mismatches (formerly `validate_tx_shape_accepts_regulatory_init_lo_non_zero`).
    #[test]
    fn reg_init_lo1_bad_shape() {
        let (sk, idx) = signer(&[32u8; 32]);
        let tx = SignedTx::sign_body(&sk, 0x2C01, idx, 0, TxBody::Init { index: 0, flags: 0 });
        let err = validate_tx_shape(&tx).expect_err("domain mismatch is expected in this fixture");
        assert!(matches!(err, TxError::DomainMismatch));
    }

    /// `BurnMark` fee amount is zero (formerly `fee_amount_is_zero_for_burn_mark`).
    #[test]
    fn fee_zero_for_burn_mark() {
        let body = TxBody::BurnMark {
            mark_amount: 1,
            beneficiary: None,
        };
        assert_eq!(body.fee_amount(), 0);
    }

    #[test]
    fn burn_purpose_rejects_control_chars() {
        let (sk, idx) = signer(&[77u8; 32]);
        let probe = SignedTx::sign_body(&sk, 0, idx, 0, TxBody::Stake { amount: 1 });
        let dom = domain_of_account_id(&probe.computed_account_id());
        let mut tx = SignedTx::sign_body(
            &sk,
            dom,
            idx,
            1,
            TxBody::BurnMark {
                mark_amount: 1,
                beneficiary: None,
            },
        );
        tx.set_burn_purpose_signed(&sk, "bad\u{0001}purpose".to_string());
        let err = validate_tx_shape(&tx).expect_err("purpose with C0 must fail");
        assert!(matches!(err, TxError::InvalidPurposeChars));
    }

    #[test]
    fn import_fee_rejects_below_minimum() {
        let (sk, idx) = signer(&[78u8; 32]);
        let probe = SignedTx::sign_body(&sk, 0, idx, 0, TxBody::Stake { amount: 1 });
        let aid = probe.computed_account_id();
        let dom = domain_of_account_id(&aid);
        let mut tx = SignedTx::sign_body(
            &sk,
            dom,
            idx,
            1,
            TxBody::Import {
                to: aid,
                amount: 10,
                export_id: [3u8; 32],
            },
        );
        tx.set_import_fee_signed(&sk, MIN_IMPORT_FEE_UNITS - 1);
        let err = validate_tx_shape(&tx).expect_err("import fee below floor must fail");
        assert!(matches!(err, TxError::ImportFeeTooLow));
    }

    /// Self-transfer shape check fails early (formerly `validate_tx_shape_rejects_self_transfer`).
    #[test]
    fn shape_reject_xfer_self() {
        let (sk, idx) = signer(&[55u8; 32]);
        let probe = SignedTx::sign_body(&sk, 0, idx, 0, TxBody::Stake { amount: 1 });
        let aid = probe.computed_account_id();
        let dom = domain_of_account_id(&aid);
        let tx = SignedTx::sign_body(
            &sk,
            dom,
            idx,
            0,
            TxBody::Transfer {
                to: aid,
                amount: 1,
                fee: 1,
            },
        );
        let err = validate_tx_shape(&tx).expect_err("self-transfer must be rejected");
        assert!(matches!(err, TxError::InvalidTransfer));
    }

    /// `export_id` stable for identical export fields (formerly `export_id_is_stable_for_identical_tx_fields`).
    #[test]
    fn export_id_stable_same_fields() {
        let (sk, idx) = signer(&[55u8; 32]);
        let probe = SignedTx::sign_body(&sk, 0, idx, 0, TxBody::Stake { amount: 1 });
        let sender_hi = domain_of_account_id(&probe.computed_account_id()).to_be_bytes()[0];
        let source_domain = ((sender_hi as u16) << 8) | 0x01;
        let target_hi = sender_hi.wrapping_add(1);
        let target_domain = ((target_hi as u16) << 8) | 0x02;
        let mut to = [0u8; 32];
        to[0] = target_hi;

        let tx1 = SignedTx::sign_body(
            &sk,
            source_domain,
            idx,
            9,
            TxBody::Export {
                to,
                target_domain,
                amount: 77,
                fee: 3,
            },
        );
        let tx2 = SignedTx::sign_body(
            &sk,
            source_domain,
            idx,
            9,
            TxBody::Export {
                to,
                target_domain,
                amount: 77,
                fee: 3,
            },
        );
        assert_eq!(tx1.export_id(), tx2.export_id());
    }

    /// Export must leave shard and `to` must match target domain (formerly `export_context_rejects_same_shard_or_wrong_target_domain`).
    #[test]
    fn export_ctx_shard_target_chk() {
        let (sk, idx) = signer(&[56u8; 32]);
        let probe = SignedTx::sign_body(&sk, 0, idx, 0, TxBody::Stake { amount: 1 });
        let sender_hi = domain_of_account_id(&probe.computed_account_id()).to_be_bytes()[0];
        let source_domain = ((sender_hi as u16) << 8) | 0x01;
        let mut to = [0u8; 32];
        to[0] = sender_hi;

        let same_shard = SignedTx::sign_body(
            &sk,
            source_domain,
            idx,
            1,
            TxBody::Export {
                to,
                target_domain: source_domain,
                amount: 10,
                fee: 1,
            },
        );
        assert!(!export_context_is_valid(&same_shard));

        let bad_target = SignedTx::sign_body(
            &sk,
            source_domain,
            idx,
            1,
            TxBody::Export {
                to,
                target_domain: source_domain.wrapping_add(1),
                amount: 10,
                fee: 1,
            },
        );
        assert!(!export_context_is_valid(&bad_target));
    }

    /// Import must target another hi-domain than signer (formerly `import_context_rejects_wrong_target_domain`).
    #[test]
    fn import_ctx_must_cross_shard() {
        let (sk, idx) = signer(&[57u8; 32]);
        let probe = SignedTx::sign_body(&sk, 0, idx, 0, TxBody::Stake { amount: 1 });
        let signer_hi = domain_of_account_id(&probe.computed_account_id()).to_be_bytes()[0];
        let signer_domain = ((signer_hi as u16) << 8) | 0x01;
        let mut to = [0u8; 32];
        to[0] = signer_hi.wrapping_add(1);
        let tx = SignedTx::sign_body(
            &sk,
            signer_domain,
            idx,
            1,
            TxBody::Import {
                to,
                amount: 10,
                export_id: [9u8; 32],
            },
        );
        assert!(!import_context_is_valid(&tx));
    }

    /// `same_hi_domain` ignores low domain byte (formerly `same_hi_domain_checks_only_hi_byte`).
    #[test]
    fn hi_byte_dom_match_only() {
        let mut a = [0u8; 32];
        let mut b = [0u8; 32];
        a[0] = 0x2C;
        a[1] = 0x01;
        b[0] = 0x2C;
        b[1] = 0xFF;
        assert!(same_hi_domain(&a, &b));
        b[0] = 0x32;
        assert!(!same_hi_domain(&a, &b));
    }

    #[test]
    fn signed_tx_json_roundtrip_u128() {
        let (sk, idx) = signer(&[91u8; 32]);
        let probe = SignedTx::sign_body(&sk, 0, idx, 0, TxBody::Stake { amount: 1 });
        let mut to = probe.computed_account_id();
        to[0] = to[0].wrapping_add(1);
        let dom = domain_of_account_id(&to);
        let tx = SignedTx::sign_body(
            &sk,
            dom,
            idx,
            7,
            TxBody::Transfer {
                to,
                amount: (u64::MAX as u128) + 123_456_789_012_345_678_u128,
                fee: (u64::MAX as u128) + 10_000_u128,
            },
        );
        let encoded = to_vec(&tx).expect("signed tx json");
        let decoded: SignedTx = from_slice(&encoded).expect("signed tx decode");
        assert_eq!(decoded, tx);
    }
}
