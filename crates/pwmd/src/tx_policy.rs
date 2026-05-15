//! Signed-tx preflight guards: shards, recipients, burns, and import provenance.

use crate::DevLane;
use axum::http::StatusCode;
use pwm_core::address_book::validate_recipient_address_policy;
use pwm_core::domain_index::{category_for_raw, DomainCategory};
use pwm_core::hd::domain_of_account_id;
use pwm_core::tx::{TxBody, TxError};
use pwm_core::SignedTx;
use tracing::info;

pub(crate) const DUPLICATE_IMPORT_ERR_TEXT: &str = "duplicate import: export_id already consumed";
const RECIPIENT_MISSING_ERR_TEXT: &str =
    "recipient account not found; recipient must run tx-init first";
const RECIPIENT_UNINIT_ERR_TEXT: &str =
    "recipient account not initialized; recipient must run tx-init first";

/// Devnet/testnet **process** shard label from Phase 1 domain class (Regulatory vs Sector).
///
/// This is intentionally **not** a protocol rule for `TRANSFER` same-shard routing.
/// Phase 1 domain classes are defined in `pwm_core::domain_index` (see RFC 1 §5.1).
pub(crate) fn shard_for_phase1_account(id: &pwm_core::AccountId) -> Result<DevLane, String> {
    let raw = domain_of_account_id(id) as u32;
    match category_for_raw(raw) {
        Some(DomainCategory::Regulatory) => Ok(DevLane::Lane0),
        Some(DomainCategory::Sector) => Ok(DevLane::Lane1),
        Some(DomainCategory::Reserve) => Err(format!(
            "account domain 0x{:04X} is reserve-class and not routable on dev shard map",
            raw as u16
        )),
        Some(DomainCategory::Witness) => Err(format!(
            "account domain 0x{:04X} is witness-class and not routable on dev shard map",
            raw as u16
        )),
        None => Err(format!(
            "account domain 0x{:04X} is unknown / not indexed for Phase1 shard map",
            raw as u16
        )),
    }
}

pub(crate) fn receiver_for_route(tx: &SignedTx) -> Option<[u8; 32]> {
    match &tx.body {
        TxBody::Transfer { to, .. } => Some(*to),
        TxBody::BurnMark {
            beneficiary: Some(to),
            ..
        } => Some(*to),
        _ => None,
    }
}

pub(crate) fn enforce_recipient_prefilter(tx: &SignedTx) -> Result<(), (StatusCode, String)> {
    // Phase 1 baseline recipient policy (reserve/witness/unknown-domain) is a separate layer from
    // shard routing; reject early at the RPC boundary for user-facing flows.
    match &tx.body {
        TxBody::Transfer { to, .. } => {
            validate_recipient_address_policy(to).map_err(|e| (StatusCode::BAD_REQUEST, e))?;
        }
        TxBody::Export { to, .. } => {
            validate_recipient_address_policy(to).map_err(|e| (StatusCode::BAD_REQUEST, e))?;
        }
        TxBody::BurnMark {
            beneficiary: Some(to),
            ..
        } => {
            validate_recipient_address_policy(to).map_err(|e| (StatusCode::BAD_REQUEST, e))?;
        }
        _ => {}
    }
    Ok(())
}

pub(crate) fn enforce_import_provenance_prefilter(
    tx: &SignedTx,
    st: &pwm_core::State,
    cross_shard: &crate::ledger::CrossShardLedger,
) -> Result<(), (StatusCode, String)> {
    let TxBody::Import {
        to,
        amount,
        export_id,
    } = &tx.body
    else {
        return Ok(());
    };
    if tx.import_fee.unwrap_or(0) < pwm_core::tx::MIN_IMPORT_FEE_UNITS {
        return Err((
            StatusCode::BAD_REQUEST,
            crate::api::common::tx_reject_json(
                tx,
                "preflight",
                &TxError::ImportFeeTooLow,
                "import fee is below MIN_IMPORT_FEE_UNITS".to_string(),
            ),
        ));
    }
    if st.imported_set.contains(export_id) {
        return Err((StatusCode::CONFLICT, DUPLICATE_IMPORT_ERR_TEXT.to_string()));
    }
    if let Some(prov) = &tx.import_provenance {
        if prov.to != *to
            || prov.amount != *amount
            || prov.target_domain.to_be_bytes()[0] != tx.domain_code.to_be_bytes()[0]
        {
            return Err((
                StatusCode::BAD_REQUEST,
                "invalid import: embedded provenance mismatch".to_string(),
            ));
        }
        if let Some(existing) = st.exported_registry.get(export_id) {
            if existing != prov {
                return Err((
                    StatusCode::BAD_REQUEST,
                    "invalid import: export provenance conflict".to_string(),
                ));
            }
            return Ok(());
        }
        let Some(fact) = cross_shard.fact(export_id) else {
            return Err((
                StatusCode::BAD_REQUEST,
                "invalid import: export_id is not known and embedded provenance is untrusted"
                    .to_string(),
            ));
        };
        if !matches!(
            fact.status,
            crate::ledger::CrossShardStatus::Registered | crate::ledger::CrossShardStatus::Imported
        ) || fact.to != prov.to
            || fact.amount != prov.amount
            || fact.target_domain_hi != tx.domain_code.to_be_bytes()[0]
        {
            return Err((
                StatusCode::BAD_REQUEST,
                "invalid import: embedded provenance mismatch".to_string(),
            ));
        }
        return Ok(());
    }
    if let Some(expected) = st.exported_registry.get(export_id) {
        if expected.to != *to
            || expected.amount != *amount
            || expected.target_domain.to_be_bytes()[0] != tx.domain_code.to_be_bytes()[0]
        {
            return Err((
                StatusCode::BAD_REQUEST,
                "invalid import: export provenance mismatch".to_string(),
            ));
        }
        return Ok(());
    }

    Err((
        StatusCode::BAD_REQUEST,
        "invalid import: export_id is not known and embedded provenance is missing".to_string(),
    ))
}

pub(crate) fn enforce_recipient_init_gate(
    tx: &SignedTx,
    st: &pwm_core::State,
) -> Result<(), (StatusCode, String)> {
    let sender = tx.computed_account_id();
    let to = match &tx.body {
        TxBody::Transfer { to, .. } if pwm_core::tx::same_hi_domain(&sender, to) => to,
        TxBody::Import { to, .. } => to,
        _ => return Ok(()),
    };
    if *to == sender {
        return Ok(());
    }
    let Some(acc) = st.get(to) else {
        return Err((
            StatusCode::BAD_REQUEST,
            RECIPIENT_MISSING_ERR_TEXT.to_string(),
        ));
    };
    if !acc.initialized {
        return Err((
            StatusCode::BAD_REQUEST,
            RECIPIENT_UNINIT_ERR_TEXT.to_string(),
        ));
    }
    Ok(())
}

pub(crate) fn enforce_local_tx_guards(
    tx: &SignedTx,
    local_shard: DevLane,
    local_domain_hi: u8,
) -> Result<(), (StatusCode, String)> {
    let sender = tx.computed_account_id();
    let sender_domain = domain_of_account_id(&sender);
    let sender_hi = sender_domain.to_be_bytes()[0];
    if sender_hi != local_domain_hi {
        return Err((
            StatusCode::CONFLICT,
            format!(
                "tx sender domain_hi=0x{sender_hi:02X} does not match node domain_hi=0x{local_domain_hi:02X}"
            ),
        ));
    }
    let sender_shard =
        shard_for_phase1_account(&sender).map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    let receiver = receiver_for_route(tx);
    let receiver_desc = receiver.map(|id| {
        let d = domain_of_account_id(&id);
        let hi = d.to_be_bytes()[0];
        let shard = shard_for_phase1_account(&id).ok();
        (hi, shard)
    });

    // Unit tests call `enforce_local_tx_guards` directly, without the HTTP prefilter layer.
    // Validate recipient policy here too so the guard returns stable `BAD_REQUEST`s.
    match &tx.body {
        TxBody::Export { to, .. } | TxBody::Import { to, .. } => {
            validate_recipient_address_policy(to).map_err(|e| (StatusCode::BAD_REQUEST, e))?;
        }
        TxBody::BurnMark {
            beneficiary: Some(to),
            ..
        } => {
            validate_recipient_address_policy(to).map_err(|e| (StatusCode::BAD_REQUEST, e))?;
        }
        _ => {}
    }
    let route_mode = match &tx.body {
        TxBody::Export {
            to,
            target_domain,
            amount: _,
            fee: _,
        } => {
            let dst_hi = target_domain.to_be_bytes()[0];
            if dst_hi == sender_hi {
                "invalid_export_same_domain"
            } else if domain_of_account_id(to).to_be_bytes()[0] != dst_hi {
                "invalid_export_recipient_domain_mismatch"
            } else {
                "export_cross_domain"
            }
        }
        TxBody::Import { .. } => "import_target_domain",
        TxBody::Transfer { .. } => {
            if let Some(to) = receiver {
                let recv_hi = domain_of_account_id(&to).to_be_bytes()[0];
                if recv_hi != sender_hi {
                    "cross_domain_requires_export_import"
                } else {
                    "same_domain_transfer"
                }
            } else {
                "same_domain_transfer"
            }
        }
        TxBody::BurnMark { beneficiary, .. } => {
            if receiver
                .map(|to| domain_of_account_id(&to).to_be_bytes()[0] != sender_hi)
                .unwrap_or(false)
            {
                "cross_domain_burn_source_only"
            } else if beneficiary.is_some()
                && receiver_desc
                    .map(|(_, s)| s.is_some_and(|s| s != sender_shard))
                    .unwrap_or(false)
            {
                "cross_shard_burn_beneficiary"
            } else {
                "local_account_op"
            }
        }
        _ => "local_account_op",
    };
    let shard_lbl = shard_label(local_domain_hi);
    info!(
        "tx routing guard: shard={} sender_domain=0x{:04X} sender_hi=0x{:02X} receiver={} mode={}",
        shard_lbl,
        sender_domain,
        sender_hi,
        receiver_desc
            .map(|(hi, shard)| format!(
                "0x{hi:02X}/{}",
                if shard.is_some() {
                    shard_label(hi)
                } else {
                    "non_routable".to_string()
                }
            ))
            .unwrap_or_else(|| "none".to_string()),
        route_mode
    );
    // Roaming txs are routed by explicit `domain_hi` identity (Sprint 13 baseline), not by the legacy
    // Phase1 process-shard map (`DevLane::Lane0|Lane1`).
    if matches!(&tx.body, TxBody::Export { .. } | TxBody::Import { .. }) {
        return Ok(());
    }
    if sender_shard != local_shard {
        return Err((
            StatusCode::CONFLICT,
            format!(
                "tx belongs to process shard {} by sender Phase1 domain class, but node runs shard {}",
                sender_shard.as_str(),
                local_shard.as_str()
            ),
        ));
    }
    if matches!(&tx.body, TxBody::Transfer { .. }) {
        let Some(to) = receiver else {
            return Ok(());
        };
        if domain_of_account_id(&to).to_be_bytes()[0] != sender_hi {
            return Err((
                StatusCode::CONFLICT,
                "cross-domain transfer is disabled on local tx path; use explicit EXPORT/IMPORT flow"
                    .to_string(),
            ));
        }
    }
    if matches!(&tx.body, TxBody::BurnMark { .. }) {
        if let Some(to) = receiver {
            let recv_shard =
                shard_for_phase1_account(&to).map_err(|e| (StatusCode::BAD_REQUEST, e))?;
            if recv_shard != sender_shard {
                return Err((
                    StatusCode::CONFLICT,
                    "cross-shard burn beneficiary is disabled on local tx path; use explicit EXPORT/IMPORT flow"
                        .to_string(),
                ));
            }
        }
    }
    Ok(())
}

/// Returns a short human-readable label for a domain-hi byte.
fn shard_label(local_domain_hi: u8) -> String {
    // Guard logs must use runtime shard labels derived from node's configured domain-hi.
    // Legacy `A|B` labels are not acceptable here (Slice20 requirement).
    pwm_core::domain_index::lookup_regulatory_by_hi(local_domain_hi)
        .map(|x| x.label.to_string())
        .unwrap_or_else(|| format!("0x{local_domain_hi:02X}"))
}

#[cfg(test)]
mod tests {
    use super::{enforce_local_tx_guards, shard_for_phase1_account};
    use crate::DevLane;
    use axum::http::StatusCode;
    use ed25519_dalek::SigningKey;
    use pwm_core::address_book::validate_recipient_address_policy;
    use pwm_core::hd::{account_id_from_parts, domain_of_account_id};
    use pwm_core::tx::{SignedTx, TxBody};
    use slip10_ed25519::derive_ed25519_private_key;

    fn user_sk(seed: &[u8; 32]) -> (SigningKey, u32, pwm_core::AccountId) {
        let sk_bytes = derive_ed25519_private_key(seed, &[0, 0]);
        let sk = SigningKey::from_bytes(&sk_bytes);
        let i = 0u32;
        let pk = sk.verifying_key().to_bytes();
        let aid = account_id_from_parts(&pk, i);
        (sk, i, aid)
    }

    fn routable_user_in_shard(
        seed_start: [u8; 32],
        want: DevLane,
        want_hi: Option<u8>,
    ) -> (SigningKey, u32, [u8; 32]) {
        let mut seed = seed_start;
        for _ in 0..16384 {
            let (sk, i, aid) = user_sk(&seed);
            if shard_for_phase1_account(&aid).ok() == Some(want)
                && want_hi
                    .map(|hi| domain_of_account_id(&aid).to_be_bytes()[0] == hi)
                    .unwrap_or(true)
                && validate_recipient_address_policy(&aid).is_ok()
            {
                return (sk, i, aid);
            }
            seed[0] = seed[0].wrapping_add(1);
        }
        panic!("failed to find routable account in shard {}", want.as_str());
    }

    fn routable_user_in_shard_opt(
        seed_start: [u8; 32],
        want: DevLane,
        want_hi: Option<u8>,
    ) -> Option<(SigningKey, u32, [u8; 32])> {
        let mut seed = seed_start;
        for _ in 0..16384 {
            let (sk, i, aid) = user_sk(&seed);
            if shard_for_phase1_account(&aid).ok() == Some(want)
                && want_hi
                    .map(|hi| domain_of_account_id(&aid).to_be_bytes()[0] == hi)
                    .unwrap_or(true)
                && validate_recipient_address_policy(&aid).is_ok()
            {
                return Some((sk, i, aid));
            }
            seed[0] = seed[0].wrapping_add(1);
        }
        None
    }

    /// Beneficiary routed by policy in same shard passes burn guard (formerly `burn_mark_guard_allows_same_shard_beneficiary`).
    #[test]
    fn burn_guard_ok_shard_ben() {
        let (sk, i, sender) = (41u8..=80u8)
            .find_map(|b| routable_user_in_shard_opt([b; 32], DevLane::Lane0, None))
            .expect("must find routable account in shard A");
        let sender_shard = shard_for_phase1_account(&sender).expect("sender shard");
        let sender_hi = domain_of_account_id(&sender).to_be_bytes()[0];
        let (_, _, beneficiary) = (60u8..=140u8)
            .find_map(|b| routable_user_in_shard_opt([b; 32], sender_shard, Some(sender_hi)))
            .expect("must find routable beneficiary in sender shard");
        let tx = SignedTx::sign_body(
            &sk,
            domain_of_account_id(&sender),
            i,
            0,
            TxBody::BurnMark {
                mark_amount: 1,
                beneficiary: Some(beneficiary),
            },
        );
        let res = enforce_local_tx_guards(&tx, sender_shard, sender_hi);
        assert!(res.is_ok());
    }

    /// Policy-invalid beneficiary id fails burn submission (formerly `burn_mark_guard_rejects_policy_invalid_beneficiary`).
    #[test]
    fn burn_guard_reject_bad_ben() {
        let (sk, i, sender) = routable_user_in_shard([51u8; 32], DevLane::Lane0, None);
        let sender_shard = shard_for_phase1_account(&sender).expect("sender shard");
        let mut bad_beneficiary = [0u8; 32];
        bad_beneficiary[0] = 0xF0;
        bad_beneficiary[1] = 0x00;
        let tx = SignedTx::sign_body(
            &sk,
            domain_of_account_id(&sender),
            i,
            0,
            TxBody::BurnMark {
                mark_amount: 1,
                beneficiary: Some(bad_beneficiary),
            },
        );
        let sender_hi = domain_of_account_id(&sender).to_be_bytes()[0];
        let err = enforce_local_tx_guards(&tx, sender_shard, sender_hi)
            .expect_err("must reject invalid beneficiary");
        assert_eq!(err.0, StatusCode::BAD_REQUEST);
    }

    /// Cross-domain beneficiary passes burn guard (V2-7: burn_ctx_source_dom removed).
    #[test]
    fn burn_guard_ok_cross_dom() {
        let (sk, i, sender) = routable_user_in_shard([61u8; 32], DevLane::Lane0, None);
        let sender_shard = shard_for_phase1_account(&sender).expect("sender shard");
        let sender_hi = domain_of_account_id(&sender).to_be_bytes()[0];
        let mut cross_domain_beneficiary = [0u8; 32];
        cross_domain_beneficiary[0] = sender_hi.wrapping_add(1);
        let tx = SignedTx::sign_body(
            &sk,
            domain_of_account_id(&sender),
            i,
            0,
            TxBody::BurnMark {
                mark_amount: 1,
                beneficiary: Some(cross_domain_beneficiary),
            },
        );
        let res = enforce_local_tx_guards(&tx, sender_shard, sender_hi);
        assert!(res.is_ok());
    }

    /// Invalid export recipient id hits HTTP BAD_REQUEST guard (formerly `export_guard_rejects_policy_invalid_recipient`).
    #[test]
    fn export_guard_reject_bad_to() {
        let (sk, i, sender) = routable_user_in_shard([71u8; 32], DevLane::Lane0, None);
        let sender_domain = domain_of_account_id(&sender);
        let sender_shard = shard_for_phase1_account(&sender).expect("sender shard");
        let sender_hi = sender_domain.to_be_bytes()[0];
        let target_hi = sender_hi.wrapping_add(1);
        let target_domain = ((target_hi as u16) << 8) | 0x01;
        let mut bad_to = [0u8; 32];
        bad_to[0] = 0xF0;
        bad_to[1] = 0x00;
        let tx = SignedTx::sign_body(
            &sk,
            sender_domain,
            i,
            0,
            TxBody::Export {
                to: bad_to,
                target_domain,
                amount: 1,
                fee: 0,
            },
        );
        let err = enforce_local_tx_guards(&tx, sender_shard, sender_hi)
            .expect_err("must reject invalid export recipient");
        assert_eq!(err.0, StatusCode::BAD_REQUEST);
    }

    /// High-byte helpers map CY/DO style labels expected in logs (formerly `shard_label_for_domain_hi_maps_to_expected_runtime_labels`).
    #[test]
    fn hi_lbl_runtime_fixture_ok() {
        assert_eq!(super::shard_label(0x2C), "CY");
        assert_eq!(super::shard_label(0x32), "DO");
    }
}
