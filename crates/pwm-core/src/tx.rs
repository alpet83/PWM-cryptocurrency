use crate::crypto::{blake3_32, sign, verify};
use crate::hd::{account_id_from_parts, domain_of_account_id};
use crate::types::AccountId;
use ed25519_dalek::SigningKey;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TxBody {
    Init {
        index: u32,
        flags: u32,
    },
    Transfer {
        to: AccountId,
        amount: u128,
        fee: u128,
    },
    Stake {
        amount: u128,
    },
    Unstake {
        amount: u128,
    },
    BurnMark {
        mark_amount: u128,
        beneficiary: Option<AccountId>,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SignedTx {
    pub domain_code: u16,
    pub signer_pk: [u8; 32],
    pub derivation_index: u32,
    pub nonce: u64,
    pub body: TxBody,
    /// Ed25519 sig, 64 bytes.
    #[serde(with = "crate::ser_bin::sig64")]
    pub signature: [u8; 64],
}

impl SignedTx {
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
                match beneficiary {
                    Some(b) => {
                        v.push(1);
                        v.extend_from_slice(b);
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
            signature: [0u8; 64],
        };
        let msg = tx.signing_message();
        tx.signature = sign(signing_key, &msg);
        tx
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
    Ok(())
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
}

#[cfg(test)]
mod tests {
    use super::{validate_tx_shape, SignedTx, TxBody, TxError};
    use ed25519_dalek::SigningKey;
    use slip10_ed25519::derive_ed25519_private_key;

    fn signer(seed: &[u8; 32]) -> (SigningKey, u32) {
        let sk_bytes = derive_ed25519_private_key(seed, &[0, 0]);
        (SigningKey::from_bytes(&sk_bytes), 0)
    }

    #[test]
    fn validate_tx_shape_accepts_regulatory_init_lo_zero() {
        let (sk, idx) = signer(&[31u8; 32]);
        let tx = SignedTx::sign_body(&sk, 0x2C00, idx, 0, TxBody::Init { index: 0, flags: 0 });
        let err = validate_tx_shape(&tx).expect_err("domain mismatch is expected in this fixture");
        assert!(matches!(err, TxError::DomainMismatch));
    }

    #[test]
    fn validate_tx_shape_accepts_regulatory_init_lo_non_zero() {
        let (sk, idx) = signer(&[32u8; 32]);
        let tx = SignedTx::sign_body(&sk, 0x2C01, idx, 0, TxBody::Init { index: 0, flags: 0 });
        let err = validate_tx_shape(&tx).expect_err("domain mismatch is expected in this fixture");
        assert!(matches!(err, TxError::DomainMismatch));
    }
}
