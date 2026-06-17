//! Signed transaction bodies, validation, and hashing helpers shared by crates.

use crate::crypto::{blake3_32, sign, verify};
use crate::hd::{account_id_from_parts, domain_of_account_id};
use crate::state::ExportProvenance;
use crate::types::{cosign_non_dis, AccountId};
use ed25519_dalek::SigningKey;
use serde::de::{self, IgnoredAny};
use serde::{Deserialize, Deserializer, Serialize};

pub const MIN_IMPORT_FEE_UNITS: u128 = 10_000;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ActivationMode {
    Dormant,
    Immediately,
    #[serde(rename = "deferred")]
    Deferred {
        activate_at_height: u64,
    },
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum PolicyKind {
    #[serde(rename = "routing.same_domain_only")]
    RoutingSameDomainOnly,
    #[serde(rename = "routing.emergency_redirect")]
    RoutingEmergencyRedirect,
    #[serde(rename = "sender_filter")]
    SenderFilter,
    #[serde(rename = "default_behavior")]
    DefaultBehavior,
    #[serde(rename = "cosign_required")]
    CosignRequired,
}

impl PolicyKind {
    pub const fn policy_id(self) -> u8 {
        match self {
            Self::RoutingSameDomainOnly => 0,
            Self::RoutingEmergencyRedirect => 1,
            Self::SenderFilter => 2,
            Self::DefaultBehavior => 3,
            Self::CosignRequired => 4,
        }
    }

    pub const fn bit(self) -> u16 {
        1u16 << self.policy_id()
    }

    pub const fn from_policy_id(id: u8) -> Option<Self> {
        match id {
            0 => Some(Self::RoutingSameDomainOnly),
            1 => Some(Self::RoutingEmergencyRedirect),
            2 => Some(Self::SenderFilter),
            3 => Some(Self::DefaultBehavior),
            4 => Some(Self::CosignRequired),
            _ => None,
        }
    }

    pub const fn is_reversible(self) -> bool {
        !matches!(self, Self::RoutingEmergencyRedirect)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PolicyAction {
    SetPolicy {
        policy: PolicyKind,
        activation: ActivationMode,
    },
    ActivatePolicy {
        policy_id: u8,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        activation_target: Option<AccountId>,
    },
    DeactivatePolicy {
        policy_id: u8,
    },
}

pub fn policy_weakens_cosign(action: &PolicyAction) -> bool {
    match action {
        PolicyAction::SetPolicy { policy, activation } => {
            *policy == PolicyKind::CosignRequired
                && !matches!(activation, ActivationMode::Immediately)
        }
        PolicyAction::DeactivatePolicy { policy_id } => {
            *policy_id == PolicyKind::CosignRequired.policy_id()
        }
        PolicyAction::ActivatePolicy { .. } => false,
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CosignRole {
    Rescue,
    Organization,
    Witness,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Cosignature {
    pub signer_pk: [u8; 32],
    pub role: CosignRole,
    #[serde(with = "crate::ser_bin::sig64")]
    pub signature: [u8; 64],
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct InitPolicyEntry {
    pub policy: PolicyKind,
    pub activation: ActivationMode,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct CosignPolicy {
    pub min_signers: u8,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct InitV4Extension {
    pub owner_kind: String,
    pub owner_display_name: String,
    pub owner_country_hint: String,
    pub company_metadata_commitment: [u8; 32],
    pub external_verification_ref: String,
    pub requested_domain_lo: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rescue_address: Option<AccountId>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub initial_policies: Vec<InitPolicyEntry>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cosign_policy: Option<CosignPolicy>,
}

pub const INIT_OWNER_KIND_MAX: usize = 32;
pub const INIT_OWNER_NAME_MAX: usize = 64;
pub const INIT_OWNER_COUNTRY_MAX: usize = 8;
pub const INIT_EXT_REF_MAX: usize = 96;
pub const INIT_MAX_POLICIES: usize = 16;

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
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
    #[serde(rename = "claim_ipv4_batch")]
    ClaimIPv4Batch {
        phase: u8,
        batch_root: [u8; 32],
        #[serde(with = "crate::ser_bin::sig64")]
        registry_sig: [u8; 64],
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
    Policy {
        target_account: AccountId,
        action: PolicyAction,
        #[serde(with = "crate::ser_json_u128")]
        fee: u128,
    },
}

impl<'de> Deserialize<'de> for TxBody {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(rename_all = "snake_case")]
        enum RawTxBody {
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
            #[serde(rename = "claim_ipv4_batch")]
            ClaimIpv4Batch {
                phase: u8,
                batch_root: [u8; 32],
                #[serde(with = "crate::ser_bin::sig64")]
                registry_sig: [u8; 64],
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
            Policy {
                target_account: AccountId,
                action: PolicyAction,
                #[serde(with = "crate::ser_json_u128")]
                fee: u128,
            },
            #[serde(rename = "claim_mark")]
            ClaimMarkRetired(IgnoredAny),
        }

        match RawTxBody::deserialize(deserializer)? {
            RawTxBody::Init { index, flags } => Ok(Self::Init { index, flags }),
            RawTxBody::Transfer { to, amount, fee } => Ok(Self::Transfer { to, amount, fee }),
            RawTxBody::Stake { amount } => Ok(Self::Stake { amount }),
            RawTxBody::Unstake { amount } => Ok(Self::Unstake { amount }),
            RawTxBody::BurnMark {
                mark_amount,
                beneficiary,
            } => Ok(Self::BurnMark {
                mark_amount,
                beneficiary,
            }),
            RawTxBody::ClaimIpv4Batch {
                phase,
                batch_root,
                registry_sig,
            } => Ok(Self::ClaimIPv4Batch {
                phase,
                batch_root,
                registry_sig,
            }),
            RawTxBody::Export {
                to,
                target_domain,
                amount,
                fee,
            } => Ok(Self::Export {
                to,
                target_domain,
                amount,
                fee,
            }),
            RawTxBody::Import {
                to,
                amount,
                export_id,
            } => Ok(Self::Import {
                to,
                amount,
                export_id,
            }),
            RawTxBody::Policy {
                target_account,
                action,
                fee,
            } => Ok(Self::Policy {
                target_account,
                action,
                fee,
            }),
            RawTxBody::ClaimMarkRetired(_) => Err(de::Error::custom(
                "tx body variant `claim_mark` is retired in V5",
            )),
        }
    }
}

impl TxBody {
    /// Canonical fee view used by policy checks and state invariants.
    /// Burn-mark flow is fixed to zero fee in Sprint 8 baseline.
    pub fn fee_amount(&self) -> u128 {
        match self {
            TxBody::Transfer { fee, .. }
            | TxBody::Export { fee, .. }
            | TxBody::Policy { fee, .. } => *fee,
            TxBody::Init { .. }
            | TxBody::Stake { .. }
            | TxBody::Unstake { .. }
            | TxBody::BurnMark { .. }
            | TxBody::ClaimIPv4Batch { .. }
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
    /// Optional V4 INIT extension fields. Allowed only with `TxBody::Init`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub init_v4: Option<InitV4Extension>,
    /// Optional additive cosign envelope for policy-gated flows.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cosigns: Vec<Cosignature>,
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
                if let Some(ext) = &self.init_v4 {
                    v.push(1);
                    push_len_prefixed_bytes(&mut v, ext.owner_kind.as_bytes());
                    push_len_prefixed_bytes(&mut v, ext.owner_display_name.as_bytes());
                    push_len_prefixed_bytes(&mut v, ext.owner_country_hint.as_bytes());
                    v.extend_from_slice(&ext.company_metadata_commitment);
                    push_len_prefixed_bytes(&mut v, ext.external_verification_ref.as_bytes());
                    v.push(ext.requested_domain_lo);
                    push_opt_account_id(&mut v, ext.rescue_address.as_ref());
                    let pol_count = u8::try_from(ext.initial_policies.len()).unwrap_or(u8::MAX);
                    v.push(pol_count);
                    for row in ext.initial_policies.iter().take(pol_count as usize) {
                        v.push(row.policy.policy_id());
                        match row.activation {
                            ActivationMode::Dormant => v.push(0),
                            ActivationMode::Immediately => v.push(1),
                            ActivationMode::Deferred { activate_at_height } => {
                                v.push(2);
                                v.extend_from_slice(&activate_at_height.to_le_bytes());
                            }
                        }
                    }
                    match &ext.cosign_policy {
                        Some(cosign) => {
                            v.push(1);
                            v.push(cosign.min_signers);
                        }
                        None => v.push(0),
                    }
                }
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
            TxBody::ClaimIPv4Batch {
                phase,
                batch_root,
                registry_sig,
            } => {
                v.push(8);
                v.push(*phase);
                v.extend_from_slice(batch_root);
                v.extend_from_slice(registry_sig);
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
            TxBody::Policy {
                target_account,
                action,
                fee,
            } => {
                v.push(9);
                v.extend_from_slice(target_account);
                push_policy_action_signing(&mut v, action);
                v.extend_from_slice(&fee.to_le_bytes());
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
            init_v4: None,
            cosigns: Vec::new(),
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

    pub fn set_init_v4_signed(&mut self, signing_key: &SigningKey, ext: Option<InitV4Extension>) {
        self.init_v4 = ext;
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
        TxBody::Init { .. } => {
            if let Some(ext) = &tx.init_v4 {
                validate_init_v4_ext(ext)?;
            }
        }
        _ if tx.init_v4.is_some() => return Err(TxError::PolicySchemaInvalid),
        TxBody::BurnMark { .. } => {
            let normalized = tx
                .burn_purpose
                .as_deref()
                .map(normalize_burn_purpose)
                .ok_or(TxError::InvalidPurposeLength)?;
            validate_burn_purpose(&normalized)?;
        }
        TxBody::ClaimIPv4Batch { registry_sig, .. } => {
            if registry_sig.iter().all(|b| *b == 0) {
                return Err(TxError::PolicySchemaInvalid);
            }
        }
        TxBody::Import { .. } => {
            if tx.import_fee.unwrap_or(0) < MIN_IMPORT_FEE_UNITS {
                return Err(TxError::ImportFeeTooLow);
            }
        }
        TxBody::Policy {
            target_account,
            action,
            fee,
        } => {
            if *target_account != aid {
                return Err(TxError::PolicySchemaInvalid);
            }
            if cosign_non_dis(&aid) && policy_weakens_cosign(action) {
                return Err(TxError::PolicyFlagNonDisableable);
            }
            match action {
                PolicyAction::SetPolicy { .. } => {
                    if *fee == 0 {
                        return Err(TxError::PolicySchemaInvalid);
                    }
                }
                PolicyAction::ActivatePolicy {
                    policy_id,
                    activation_target,
                } => {
                    if *fee != 0 {
                        return Err(TxError::PolicyActivationFeeMustBeZero);
                    }
                    let Some(pol) = PolicyKind::from_policy_id(*policy_id) else {
                        return Err(TxError::PolicySchemaInvalid);
                    };
                    if pol == PolicyKind::RoutingEmergencyRedirect && activation_target.is_none() {
                        return Err(TxError::PolicyActivationTargetRequired);
                    }
                    if pol != PolicyKind::RoutingEmergencyRedirect && activation_target.is_some() {
                        return Err(TxError::PolicyActivationTargetNotAllowed);
                    }
                }
                PolicyAction::DeactivatePolicy { policy_id } => {
                    if *fee == 0 {
                        return Err(TxError::PolicySchemaInvalid);
                    }
                    if PolicyKind::from_policy_id(*policy_id).is_none() {
                        return Err(TxError::PolicySchemaInvalid);
                    }
                }
            }
        }
        _ => {}
    }
    Ok(())
}

fn validate_init_v4_ext(ext: &InitV4Extension) -> Result<(), TxError> {
    if ext.owner_kind.is_empty() || ext.owner_kind.as_bytes().len() > INIT_OWNER_KIND_MAX {
        return Err(TxError::PolicySchemaInvalid);
    }
    if ext.owner_display_name.is_empty()
        || ext.owner_display_name.as_bytes().len() > INIT_OWNER_NAME_MAX
    {
        return Err(TxError::PolicySchemaInvalid);
    }
    if ext.owner_country_hint.is_empty()
        || ext.owner_country_hint.as_bytes().len() > INIT_OWNER_COUNTRY_MAX
    {
        return Err(TxError::PolicySchemaInvalid);
    }
    if ext.external_verification_ref.as_bytes().len() > INIT_EXT_REF_MAX {
        return Err(TxError::PolicySchemaInvalid);
    }
    if ext.initial_policies.len() > INIT_MAX_POLICIES {
        return Err(TxError::PolicySchemaInvalid);
    }
    for row in &ext.initial_policies {
        if row.policy.policy_id() >= 16 {
            return Err(TxError::PolicySchemaInvalid);
        }
    }
    Ok(())
}

fn push_len_prefixed_bytes(out: &mut Vec<u8>, payload: &[u8]) {
    let len = u16::try_from(payload.len()).unwrap_or(u16::MAX);
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(payload);
}

fn push_opt_account_id(out: &mut Vec<u8>, value: Option<&AccountId>) {
    match value {
        Some(id) => {
            out.push(1);
            out.extend_from_slice(id);
        }
        None => out.push(0),
    }
}

fn push_policy_action_signing(out: &mut Vec<u8>, action: &PolicyAction) {
    match action {
        PolicyAction::SetPolicy { policy, activation } => {
            out.push(0);
            out.push(policy.policy_id());
            match activation {
                ActivationMode::Dormant => out.push(0),
                ActivationMode::Immediately => out.push(1),
                ActivationMode::Deferred { activate_at_height } => {
                    out.push(2);
                    out.extend_from_slice(&activate_at_height.to_le_bytes());
                }
            }
        }
        PolicyAction::ActivatePolicy {
            policy_id,
            activation_target,
        } => {
            out.push(1);
            out.push(*policy_id);
            push_opt_account_id(out, activation_target.as_ref());
        }
        PolicyAction::DeactivatePolicy { policy_id } => {
            out.push(2);
            out.push(*policy_id);
        }
    }
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
    #[error("export lock refunded")]
    ExportLockRefunded,
    #[error("invalid purpose length")]
    InvalidPurposeLength,
    #[error("invalid purpose chars")]
    InvalidPurposeChars,
    #[error("unsupported tx kind")]
    UnsupportedTxKind,
    #[error("import fee too low")]
    ImportFeeTooLow,
    #[error("policy schema invalid")]
    PolicySchemaInvalid,
    #[error("policy not installed")]
    PolicyNotInstalled,
    #[error("policy not active")]
    PolicyNotActive,
    #[error("policy denied")]
    PolicyDenied,
    #[error("policy sender filtered")]
    PolicySenderFiltered,
    #[error("policy routing denied")]
    PolicyRoutingDenied,
    #[error("policy missing cosign")]
    PolicyMissingCosign,
    #[error("policy rescue required")]
    PolicyRescueRequired,
    #[error("policy emergency cosign required")]
    PolicyEmergencyCosignRequired,
    #[error("policy account finalized")]
    PolicyAccountFinalized,
    #[error("policy irreversible")]
    PolicyIrreversible,
    #[error("policy flag non-disableable")]
    PolicyFlagNonDisableable,
    #[error("conservation delay required")]
    ConservationDelayRequired,
    #[error("conservation pending exists")]
    ConservationPendingExists,
    #[error("policy activation fee must be zero")]
    PolicyActivationFeeMustBeZero,
    #[error("policy activation target mismatch")]
    PolicyActivationTargetMismatch,
    #[error("policy activation target required")]
    PolicyActivationTargetRequired,
    #[error("policy activation target not allowed")]
    PolicyActivationTargetNotAllowed,
    #[error("evidence duplicate")]
    EvidenceDuplicate,
}

#[cfg(test)]
mod tests {
    use super::{
        export_context_is_valid, import_context_is_valid, same_hi_domain, validate_tx_shape,
        ActivationMode, InitPolicyEntry, InitV4Extension, PolicyAction, PolicyKind, SignedTx,
        TxBody, TxError, MIN_IMPORT_FEE_UNITS,
    };
    use crate::hd::domain_of_account_id;
    use crate::types::cosign_non_dis;
    use ed25519_dalek::SigningKey;
    use serde_json::{from_slice, to_vec};
    use slip10_ed25519::derive_ed25519_private_key;

    fn signer(seed: &[u8; 32]) -> (SigningKey, u32) {
        let sk_bytes = derive_ed25519_private_key(seed, &[0, 0]);
        (SigningKey::from_bytes(&sk_bytes), 0)
    }

    fn signer_no_flag(seed_start: u8) -> (SigningKey, u32) {
        for attempt in 0..=255 {
            let mut seed = [seed_start; 32];
            seed[0] = seed_start.wrapping_add(attempt);
            let (sk, idx) = signer(&seed);
            let probe = SignedTx::sign_body(&sk, 0, idx, 0, TxBody::Stake { amount: 1 });
            if !cosign_non_dis(&probe.computed_account_id()) {
                return (sk, idx);
            }
        }
        panic!("failed to find non-flagged signer");
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
        let (sk, idx) = signer_no_flag(78);
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

    #[test]
    fn policy_tx_json_fee_str() {
        let (sk, idx) = signer(&[92u8; 32]);
        let probe = SignedTx::sign_body(&sk, 0, idx, 0, TxBody::Stake { amount: 1 });
        let aid = probe.computed_account_id();
        let dom = domain_of_account_id(&aid);
        let tx = SignedTx::sign_body(
            &sk,
            dom,
            idx,
            3,
            TxBody::Policy {
                target_account: aid,
                action: PolicyAction::SetPolicy {
                    policy: PolicyKind::SenderFilter,
                    activation: ActivationMode::Dormant,
                },
                fee: (u64::MAX as u128) + 123_456_789,
            },
        );
        let fee_txt = ((u64::MAX as u128) + 123_456_789).to_string();
        let json = String::from_utf8(to_vec(&tx).expect("json")).expect("utf8");
        assert!(json.contains(&format!("\"fee\":\"{fee_txt}\"")));
        let decoded: SignedTx = from_slice(json.as_bytes()).expect("decode");
        assert_eq!(decoded, tx);
    }

    #[test]
    fn claim_ipv4_batch_signing_json() {
        let (sk, idx) = signer(&[95u8; 32]);
        let probe = SignedTx::sign_body(&sk, 0, idx, 0, TxBody::Stake { amount: 1 });
        let dom = domain_of_account_id(&probe.computed_account_id());
        let tx = SignedTx::sign_body(
            &sk,
            dom,
            idx,
            8,
            TxBody::ClaimIPv4Batch {
                phase: 3,
                batch_root: [0xAB; 32],
                registry_sig: [0xCD; 64],
            },
        );
        let encoded = to_vec(&tx).expect("json");
        let decoded: SignedTx = from_slice(&encoded).expect("decode");
        assert_eq!(decoded, tx);
        assert_eq!(tx.signing_message(), decoded.signing_message());
    }

    #[test]
    fn claim_ipv4_rejects_zero_sig() {
        let (sk, idx) = signer(&[96u8; 32]);
        let probe = SignedTx::sign_body(&sk, 0, idx, 0, TxBody::Stake { amount: 1 });
        let dom = domain_of_account_id(&probe.computed_account_id());
        let tx = SignedTx::sign_body(
            &sk,
            dom,
            idx,
            9,
            TxBody::ClaimIPv4Batch {
                phase: 1,
                batch_root: [0x11; 32],
                registry_sig: [0u8; 64],
            },
        );
        let err = validate_tx_shape(&tx).expect_err("zero registry sig must fail shape validation");
        assert!(matches!(err, TxError::PolicySchemaInvalid));
    }

    #[test]
    fn claim_mark_wire_retired_error() {
        let raw = r#"{"claim_mark":{"mode":"free","claim_units":"1","anchor_ref":0,"fee":"0"}}"#;
        let err = from_slice::<TxBody>(raw.as_bytes()).expect_err("claim_mark must be retired");
        assert!(err.to_string().contains("retired in V5"));
        assert!(err.to_string().contains("claim_mark"));
    }

    #[test]
    fn policy_signing_changes_by_action() {
        let (sk, idx) = signer(&[93u8; 32]);
        let probe = SignedTx::sign_body(&sk, 0, idx, 0, TxBody::Stake { amount: 1 });
        let aid = probe.computed_account_id();
        let dom = domain_of_account_id(&aid);
        let tx_a = SignedTx::sign_body(
            &sk,
            dom,
            idx,
            4,
            TxBody::Policy {
                target_account: aid,
                action: PolicyAction::ActivatePolicy {
                    policy_id: PolicyKind::CosignRequired.policy_id(),
                    activation_target: None,
                },
                fee: 0,
            },
        );
        let tx_b = SignedTx::sign_body(
            &sk,
            dom,
            idx,
            4,
            TxBody::Policy {
                target_account: aid,
                action: PolicyAction::DeactivatePolicy {
                    policy_id: PolicyKind::CosignRequired.policy_id(),
                },
                fee: 10,
            },
        );
        assert_ne!(tx_a.signing_message(), tx_b.signing_message());
    }

    #[test]
    fn pol_deferred_signing_by_height() {
        let (sk, idx) = signer(&[97u8; 32]);
        let probe = SignedTx::sign_body(&sk, 0, idx, 0, TxBody::Stake { amount: 1 });
        let aid = probe.computed_account_id();
        let dom = domain_of_account_id(&aid);
        let tx_a = SignedTx::sign_body(
            &sk,
            dom,
            idx,
            5,
            TxBody::Policy {
                target_account: aid,
                action: PolicyAction::SetPolicy {
                    policy: PolicyKind::SenderFilter,
                    activation: ActivationMode::Deferred {
                        activate_at_height: 100,
                    },
                },
                fee: 10,
            },
        );
        let tx_b = SignedTx::sign_body(
            &sk,
            dom,
            idx,
            5,
            TxBody::Policy {
                target_account: aid,
                action: PolicyAction::SetPolicy {
                    policy: PolicyKind::SenderFilter,
                    activation: ActivationMode::Deferred {
                        activate_at_height: 101,
                    },
                },
                fee: 10,
            },
        );
        assert_ne!(tx_a.signing_message(), tx_b.signing_message());
    }

    #[test]
    fn policy_deferred_json_roundtrip() {
        let (sk, idx) = signer(&[98u8; 32]);
        let probe = SignedTx::sign_body(&sk, 0, idx, 0, TxBody::Stake { amount: 1 });
        let aid = probe.computed_account_id();
        let dom = domain_of_account_id(&aid);
        let tx = SignedTx::sign_body(
            &sk,
            dom,
            idx,
            6,
            TxBody::Policy {
                target_account: aid,
                action: PolicyAction::SetPolicy {
                    policy: PolicyKind::SenderFilter,
                    activation: ActivationMode::Deferred {
                        activate_at_height: 123,
                    },
                },
                fee: 42,
            },
        );
        let encoded = to_vec(&tx).expect("json");
        let decoded: SignedTx = from_slice(&encoded).expect("decode");
        assert_eq!(decoded, tx);
    }

    #[test]
    fn pol_activate_target_json_roundtrip() {
        let (sk, idx) = signer(&[99u8; 32]);
        let probe = SignedTx::sign_body(&sk, 0, idx, 0, TxBody::Stake { amount: 1 });
        let aid = probe.computed_account_id();
        let dom = domain_of_account_id(&aid);
        let mut tgt = aid;
        tgt[31] = tgt[31].wrapping_add(1);
        let tx = SignedTx::sign_body(
            &sk,
            dom,
            idx,
            7,
            TxBody::Policy {
                target_account: aid,
                action: PolicyAction::ActivatePolicy {
                    policy_id: PolicyKind::SenderFilter.policy_id(),
                    activation_target: Some(tgt),
                },
                fee: 11,
            },
        );
        let encoded = to_vec(&tx).expect("json");
        let decoded: SignedTx = from_slice(&encoded).expect("decode");
        assert_eq!(decoded, tx);
    }

    #[test]
    fn pol_act_tgt_json_legacy() {
        let raw = r#"{
            "domain_code":0,
            "signer_pk":[0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],
            "derivation_index":0,
            "nonce":0,
            "body":{
                "policy":{
                    "target_account":[0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],
                    "action":{"activate_policy":{"policy_id":2}},
                    "fee":"1"
                }
            },
            "cosigns":[],
            "signature":[0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0]
        }"#;
        let tx: SignedTx = from_slice(raw.as_bytes()).expect("legacy activation_target omitted");
        let TxBody::Policy { action, .. } = tx.body else {
            panic!("policy body expected");
        };
        let PolicyAction::ActivatePolicy {
            policy_id,
            activation_target,
        } = action
        else {
            panic!("activate policy action expected");
        };
        assert_eq!(policy_id, 2);
        assert_eq!(activation_target, None);
    }

    #[test]
    fn pol_activate_target_signing_diff() {
        let (sk, idx) = signer(&[100u8; 32]);
        let probe = SignedTx::sign_body(&sk, 0, idx, 0, TxBody::Stake { amount: 1 });
        let aid = probe.computed_account_id();
        let dom = domain_of_account_id(&aid);
        let mut tgt = aid;
        tgt[0] = tgt[0].wrapping_add(1);
        let tx_a = SignedTx::sign_body(
            &sk,
            dom,
            idx,
            8,
            TxBody::Policy {
                target_account: aid,
                action: PolicyAction::ActivatePolicy {
                    policy_id: PolicyKind::SenderFilter.policy_id(),
                    activation_target: None,
                },
                fee: 10,
            },
        );
        let tx_b = SignedTx::sign_body(
            &sk,
            dom,
            idx,
            8,
            TxBody::Policy {
                target_account: aid,
                action: PolicyAction::ActivatePolicy {
                    policy_id: PolicyKind::SenderFilter.policy_id(),
                    activation_target: Some(tgt),
                },
                fee: 0,
            },
        );
        assert_ne!(tx_a.signing_message(), tx_b.signing_message());
    }

    #[test]
    fn pol_act_tgt_non_emerg() {
        let (sk, idx) = signer_no_flag(101);
        let probe = SignedTx::sign_body(&sk, 0, idx, 0, TxBody::Stake { amount: 1 });
        let aid = probe.computed_account_id();
        let dom = domain_of_account_id(&aid);
        let mut tgt = aid;
        tgt[31] = tgt[31].wrapping_add(1);
        let tx = SignedTx::sign_body(
            &sk,
            dom,
            idx,
            8,
            TxBody::Policy {
                target_account: aid,
                action: PolicyAction::ActivatePolicy {
                    policy_id: PolicyKind::SenderFilter.policy_id(),
                    activation_target: Some(tgt),
                },
                fee: 0,
            },
        );
        let err = validate_tx_shape(&tx).expect_err("non-emergency policy must reject target");
        assert!(
            matches!(err, TxError::PolicyActivationTargetNotAllowed),
            "{err:?}"
        );
    }

    #[test]
    fn policy_deferred_json_requires_height() {
        let raw = r#"{
            "domain_code":0,
            "signer_pk":[0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],
            "derivation_index":0,
            "nonce":0,
            "body":{
                "policy":{
                    "target_account":[0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],
                    "action":{"set_policy":{"policy":"sender_filter","activation":"deferred"}},
                    "fee":"1"
                }
            },
            "cosigns":[],
            "signature":[0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0]
        }"#;
        assert!(from_slice::<SignedTx>(raw.as_bytes()).is_err());
    }

    #[test]
    fn init_v4_signing_json() {
        let (sk, idx) = signer(&[94u8; 32]);
        let probe = SignedTx::sign_body(&sk, 0, idx, 0, TxBody::Stake { amount: 1 });
        let dom = domain_of_account_id(&probe.computed_account_id());
        let mut tx = SignedTx::sign_body(&sk, dom, idx, 0, TxBody::Init { index: 1, flags: 2 });
        let base = tx.signing_message();
        tx.set_init_v4_signed(
            &sk,
            Some(InitV4Extension {
                owner_kind: "company".to_string(),
                owner_display_name: "Acme".to_string(),
                owner_country_hint: "CY".to_string(),
                company_metadata_commitment: [7u8; 32],
                external_verification_ref: "https://example.org/ref".to_string(),
                requested_domain_lo: 0,
                rescue_address: None,
                initial_policies: vec![InitPolicyEntry {
                    policy: PolicyKind::RoutingSameDomainOnly,
                    activation: ActivationMode::Immediately,
                }],
                cosign_policy: None,
            }),
        );
        assert_ne!(base, tx.signing_message());
        let encoded = to_vec(&tx).expect("json");
        let decoded: SignedTx = from_slice(&encoded).expect("decode");
        assert_eq!(decoded, tx);
    }
}
